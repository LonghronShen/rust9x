//! WSPiApi.h getaddr/freeaddrinfo shim converted to rust

use crate::{
    ffi::CStr,
    ptr,
    sys::c::{
        in_addr, sockaddr_in, WSAGetLastError, ADDRESS_FAMILY, ADDRINFOA, AF_INET, SOCK_DGRAM,
        SOCK_STREAM, USHORT,
    },
};
use libc::{c_char, c_int, c_ulong};

const WSABASEERR: c_int = 10000;
const WSAHOST_NOT_FOUND: c_int = WSABASEERR + 1001;
const WSATRY_AGAIN: c_int = WSABASEERR + 1002;
const WSANO_RECOVERY: c_int = WSABASEERR + 1003;
const WSANO_DATA: c_int = WSABASEERR + 1004;

const EAI_NONAME: c_int = WSAHOST_NOT_FOUND;

// https://lists.freebsd.org/pipermail/freebsd-ports/2003-October/005757.html
const EAI_NODATA: c_int = EAI_NONAME;
const EAI_AGAIN: c_int = WSATRY_AGAIN;
const EAI_FAIL: c_int = WSANO_RECOVERY;
const EAI_BADFLAGS: c_int = 10022;
const EAI_FAMILY: c_int = 10047;
const EAI_SOCKTYPE: c_int = 10044;
const EAI_SERVICE: c_int = 10109;

const WSA_NOT_ENOUGH_MEMORY: c_int = 8;
const EAI_MEMORY: c_int = WSA_NOT_ENOUGH_MEMORY;

const AI_PASSIVE: i32 = 0x00000001;
const AI_CANONNAME: i32 = 0x00000002;
const AI_NUMERICHOST: i32 = 0x00000004;

const PF_UNSPEC: i32 = 0;
const PF_INET: i32 = 2;

const SOCK_RAW: i32 = 3;

const INADDR_ANY: u32 = 0x00000000;
const INADDR_LOOPBACK: u32 = 0x7f000001;

const NI_MAXHOST: usize = 1025;

pub unsafe fn wspiapi_freeaddrinfo(mut head: *mut ADDRINFOA) {
    let mut next_ptr = head;

    while !next_ptr.is_null() {
        // scope to make sure the `next` borrow is dropped before freeeing the `ADDRINFOA` it
        // references
        {
            let next = &*next_ptr;
            if !next.ai_canonname.is_null() {
                drop(crate::ffi::CString::from_raw(next.ai_canonname));
            }

            if !next.ai_addr.is_null() {
                drop(Box::<sockaddr_in>::from_raw(next.ai_addr as *mut _));
            }

            head = next.ai_next;
        }

        drop(Box::<ADDRINFOA>::from_raw(next_ptr));
        next_ptr = head;
    }
}

/// Protocol-independent name-to-address translation.
///
/// As specified in RFC 2553, Section 6.4.
/// This is the hacked version that only supports IPv4.
///
/// Arguments
/// -   node              node name to lookup.
/// -   service           service name to lookup.
/// -   hints             hints about how to process request.
/// -   res               where to return result.
///
/// Return Value
/// -   returns zero if successful, an EAI_* error code if not.
pub unsafe fn wspiapi_getaddrinfo(
    node: *const c_char,
    service: *const c_char,
    hints: *const ADDRINFOA,
    res: *mut *mut ADDRINFOA,
) -> c_int {
    // initialize res with default return value.
    *res = ptr::null_mut();

    // the node name and the service name can't both be NULL.
    if node.is_null() && service.is_null() {
        return EAI_NONAME;
    }

    let mut flags: i32 = 0;
    let mut socket_type: i32 = 0;
    let mut protocol: i32 = 0;

    // validate hints.
    if let Some(hints) = ptr::NonNull::<ADDRINFOA>::new(hints as *mut _) {
        let hints = hints.as_ref();

        // all members other than ai_flags, ai_family, ai_socktype
        // and ai_protocol must be zero or a null pointer.
        if hints.ai_addrlen != 0
            || !hints.ai_canonname.is_null()
            || !hints.ai_addr.is_null()
            || !hints.ai_next.is_null()
        {
            return EAI_FAIL;
        }

        // the spec has the "bad flags" error code, so presumably we
        // should check something here.  insisting that there aren't
        // any unspecified flags set would break forward compatibility,
        // however.  so we just check for non-sensical combinations.
        //
        // we cannot come up with a canonical name given a null node name.
        flags = hints.ai_flags;
        if flags & AI_CANONNAME != 0 && node.is_null() {
            return EAI_BADFLAGS;
        }

        // we only support a limited number of protocol families.
        if !matches!(hints.ai_family, PF_UNSPEC | PF_INET) {
            return EAI_FAMILY;
        }

        // we only support only these socket types.
        socket_type = hints.ai_socktype;
        if !matches!(socket_type, 0 | SOCK_STREAM | SOCK_DGRAM | SOCK_RAW) {
            return EAI_SOCKTYPE;
        }

        // REVIEW: What if ai_socktype and ai_protocol are at odds?
        protocol = hints.ai_protocol;
    }

    let mut port: USHORT = 0;
    let mut udp_port: USHORT = 0;
    let mut clone: bool = false;

    // do service lookup
    if !service.is_null() {
        if let Some(raw_port) =
            CStr::from_ptr(service).to_str().ok().and_then(|s| s.parse::<c_ulong>().ok())
        {
            // numeric port string

            port = (raw_port as USHORT).to_be();
            udp_port = port;

            if socket_type == 0 {
                clone = true;
                socket_type = SOCK_STREAM;
            }
        } else {
            let mut tcp_port: USHORT = 0;

            // non numeric port string

            if socket_type == 0 || socket_type == SOCK_DGRAM {
                let servent = getservbyname(service, b"udp\0".as_ptr() as *const c_char);
                if !servent.is_null() {
                    port = (*servent).s_port;
                    udp_port = port;
                }
            }

            if socket_type == 0 || socket_type == SOCK_STREAM {
                let servent = getservbyname(service, b"tcp\0".as_ptr() as *const c_char);
                if !servent.is_null() {
                    port = (*servent).s_port;
                    tcp_port = port;
                }
            }

            // assumes 0 is an invalid service port...
            if port == 0 {
                // no service exists
                return if socket_type != 0 { EAI_SERVICE } else { EAI_NONAME };
            }

            if socket_type == 0 {
                // if both tcp and udp, process tcp now & clone udp later.
                socket_type = if tcp_port != 0 { SOCK_STREAM } else { SOCK_DGRAM };
                clone = tcp_port != 0 && udp_port != 0;
            }
        }
    }

    // do node name lookup

    // if we weren't given a node name,
    // return the wildcard or loopback address (depending on AI_PASSIVE).
    //
    // if we have a numeric host address string,
    // return the binary address.
    //

    let address: Option<u32> = if node.is_null() {
        Some((if flags & AI_PASSIVE != 0 { INADDR_ANY } else { INADDR_LOOPBACK }).to_be())
    } else {
        wspiapi_parse_v4_address(CStr::from_ptr(node))
    };

    let mut error: i32 = 0;

    if let Some(address) = address {
        // create an addrinfo structure...
        *res = wspiapi_new_addr_info(socket_type, protocol, port, address);

        if error != 0 && !node.is_null() {
            // implementation specific behavior: set AI_NUMERICHOST
            // to indicate that we got a numeric host address string.
            (**res).ai_flags |= AI_NUMERICHOST;

            // return the numeric address string as the canonical name
            if flags & AI_CANONNAME != 0 {
                (**res).ai_canonname = wspiapi_strdup(inet_ntoa(in_addr { s_addr: address }));

                if (**res).ai_canonname.is_null() {
                    error = EAI_MEMORY;
                }
            }
        }
    } else if flags & AI_NUMERICHOST != 0 {
        // if we do not have a numeric host address string and
        // AI_NUMERICHOST flag is set, return an error!
        error = EAI_NONAME;
    } else {
        // since we have a non-numeric node name,
        // we have to do a regular node name lookup.
        error = wspiapi_lookup_node(
            CStr::from_ptr(node),
            socket_type,
            protocol,
            port,
            flags & AI_CANONNAME != 0,
            res,
        );
    }

    if error == 0 && clone {
        error = wspiapi_clone(udp_port, *res);
    }

    if error != 0 {
        wspiapi_freeaddrinfo(*res);
        *res = ptr::null_mut();
    }

    return error;
}

unsafe fn wspiapi_clone(udp_port: USHORT, res: *mut ADDRINFOA) -> i32 {
    let mut next_ptr = res;

    while !next_ptr.is_null() {
        let next = &mut *next_ptr;

        // create an addrinfo structure...
        let new_ptr = wspiapi_new_addr_info(
            SOCK_DGRAM,
            next.ai_protocol,
            udp_port,
            (*(next.ai_addr as *mut sockaddr_in)).sin_addr.s_addr,
        );
        let new = &mut *new_ptr;

        // link the cloned addrinfo
        new.ai_next = next.ai_next;
        next.ai_next = new_ptr;
        next_ptr = new.ai_next;
    }

    0
}

/// Resolve a nodename and return a list of addrinfo structures.
/// IPv4 specific internal function, not exported.
///
/// *res would need to be freed if an error is returned.
///
/// NOTE: if `ai_canonname` is true, the canonical name should be
///       returned in the first addrinfo structure.
///
/// Arguments
/// - node                name of node to resolve.
/// - socket_type         SOCK_*.  can be wildcarded (zero).
/// - protocol            IPPROTO_*.  can be wildcarded (zero).
/// - port                port number of service (in network order).
/// - ai_canonname        whether the AI_CANONNAME flag is set.
/// - res                 where to return result.
///
/// Return Value
/// - Returns 0 on success, an EAI_* style error value otherwise.
unsafe fn wspiapi_lookup_node(
    node: &CStr,
    socket_type: i32,
    protocol: i32,
    port: USHORT,
    ai_canonname: bool,
    res: *mut *mut ADDRINFOA,
) -> i32 {
    let mut error: i32;
    let mut alias_count = 0;

    let mut name = [0u8; NI_MAXHOST];
    wspiapi_strcpy_ni_maxhost(&mut name, node.to_bytes());

    let mut alias = [0u8; NI_MAXHOST];

    let mut name_ref = &mut name;
    let mut alias_ref = &mut alias;

    loop {
        error = wspiapi_query_dns(node, socket_type, protocol, port, alias_ref, res);

        if error != 0 {
            break;
        }

        if (*res).is_null() {
            break;
        }

        if alias_ref[0] == b'\0'
            || CStr::from_ptr(name_ref.as_ptr() as *const _)
                == CStr::from_ptr(alias_ref.as_ptr() as *const _)
            || {
                alias_count += 1;
                alias_count
            } == 16
        {
            error = EAI_FAIL;
            break;
        }

        crate::mem::swap(&mut name_ref, &mut alias_ref);
    }

    if error == 0 && ai_canonname {
        (**res).ai_canonname = wspiapi_strdup(alias_ref.as_ptr() as *const i8);
    }

    error
}

fn wspiapi_strcpy_ni_maxhost(dest: &mut [u8; NI_MAXHOST], source_without_nul: &[u8]) {
    let len = source_without_nul.len().min(NI_MAXHOST - 1);
    dest[0..len].copy_from_slice(&source_without_nul[0..len]);
    dest[len] = b'\0';
}

unsafe fn wspiapi_query_dns(
    node: &CStr,
    socket_type: i32,
    protocol: i32,
    port: USHORT,
    alias_ref: &mut [u8; NI_MAXHOST],
    res: *mut *mut ADDRINFOA,
) -> i32 {
    let mut next = res;

    alias_ref[0] = b'\0';

    let host = gethostbyname(node.as_ptr());
    if let Some(host) = ptr::NonNull::<hostent>::new(host as *mut _) {
        let host = host.as_ref();

        if host.h_addrtype == AF_INET as USHORT
            && host.h_length == crate::mem::size_of::<in_addr>() as USHORT
        {
            let mut addresses = host.h_addr_list;

            while !addresses.is_null() {
                *next = wspiapi_new_addr_info(
                    socket_type,
                    protocol,
                    port,
                    (*((*addresses) as *const in_addr)).s_addr,
                );

                next = ptr::addr_of_mut!((**next).ai_next);

                addresses = addresses.add(1);
            }
        }

        wspiapi_strcpy_ni_maxhost(alias_ref, CStr::from_ptr(host.h_name).to_bytes());

        return 0;
    }

    match WSAGetLastError() {
        WSAHOST_NOT_FOUND => EAI_NONAME,
        WSATRY_AGAIN => EAI_AGAIN,
        WSANO_RECOVERY => EAI_FAIL,
        WSANO_DATA => EAI_NODATA,
        _ => EAI_NONAME,
    }
}

unsafe fn wspiapi_new_addr_info(
    socket_type: i32,
    protocol: i32,
    port: USHORT,
    address: u32,
) -> *mut ADDRINFOA {
    let sockaddr = box sockaddr_in {
        sin_family: AF_INET as ADDRESS_FAMILY,
        sin_port: port,
        sin_addr: in_addr { s_addr: address },
        sin_zero: [0; 8],
    };

    let new = box ADDRINFOA {
        ai_family: PF_INET,
        ai_socktype: socket_type,
        ai_protocol: protocol,
        ai_addrlen: crate::mem::size_of::<sockaddr_in>(),
        ai_addr: Box::into_raw(sockaddr) as *mut _,
        ai_canonname: ptr::null_mut(),
        ai_flags: 0,
        ai_next: ptr::null_mut(),
    };

    Box::into_raw(new)
}

/// Get the IPv4 address (in network byte order) from its string representation.
/// The syntax should be `a.b.c.d`.
///
/// Arguments
/// - pszArgument         string representation of the IPv4 address
/// - ptAddress           pointer to the resulting IPv4 address
///
/// Return Value
/// - Returns FALSE if there is an error, TRUE for success.
fn wspiapi_parse_v4_address(address: &CStr) -> Option<u32> {
    // ensure there are 3 '.' (periods)
    if address.to_bytes().iter().filter(|&&c| c == b'.').count() != 3 {
        return None;
    }

    // return an error if dwAddress is INADDR_NONE (255.255.255.255)
    // since this is never a valid argument to getaddrinfo.
    let addr: u32 = unsafe { inet_addr(address.as_ptr()) };

    const INADDR_NONE: u32 = 0xffffffff;
    if addr == INADDR_NONE {
        return None;
    }

    return Some(addr);
}

#[inline]
unsafe fn wspiapi_strdup(string: *const c_char) -> *mut c_char {
    if string.is_null() { ptr::null_mut() } else { CStr::from_ptr(string).to_owned().into_raw() }
}

// from Winsock2.h
#[repr(C)]
pub struct servent {
    s_name: *mut c_char,
    s_aliases: *mut *mut c_char,
    #[cfg(target_pointer_width = "32")]
    s_port: USHORT,
    #[cfg(target_pointer_width = "32")]
    s_proto: *mut c_char,
    #[cfg(target_pointer_width = "64")]
    s_proto: *mut c_char,
    #[cfg(target_pointer_width = "64")]
    s_port: USHORT,
}

#[repr(C)]
pub struct hostent {
    h_name: *const c_char,
    h_aliases: *const *const c_char,
    h_addrtype: USHORT,
    h_length: USHORT,
    h_addr_list: *const *const c_char,
}

compat_fn_lazy! {
    "ws2_32":{unicows: false, load: true}:
    /// The pointer that is returned points to the SERVENT structure allocated by the
    /// Windows Sockets library. The application must never attempt to modify this
    /// structure or to free any of its components. Furthermore only one copy of this
    /// structure is allocated per thread, so the application should copy any information
    /// it needs before issuing any other Windows Sockets function calls.
    pub fn getservbyname(name: *const c_char, proto: *const c_char) -> *const servent {
        panic!("unavailable")
    }
    /// The `gethostbyname` function returns a pointer to a hostent structure—a structure allocated
    /// by Windows Sockets. The hostent structure contains the results of a successful search for
    /// the host specified in the name parameter.
    ///
    /// The application must never attempt to modify this structure or to free any of its
    /// components. Furthermore, only one copy of this structure is allocated per thread, so the
    /// application should copy any information it needs before issuing any other Windows Sockets
    /// function calls.
    pub fn gethostbyname(name: *const c_char) -> *const hostent {
        panic!("unavailable")
    }
    pub fn inet_addr(cp: *const c_char) -> u32 {
        panic!("unavailable")
    }
    pub fn inet_ntoa(r#in: in_addr) -> *const c_char {
        panic!("unavailable")
    }
}
