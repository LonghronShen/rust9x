//! WSPiApi.h freeaddrinfo/freeaddrinfo shim converted to rust
//!
//! Basically a 1:1 translation from C, just with regular rust/box allocation
//! instead of calloc

use crate::{
    ffi::CStr,
    sys::{
        c::{
            in_addr, sockaddr_in, WSAGetLastError, ADDRESS_FAMILY, ADDRINFOA, AF_INET, BOOL,
            HMODULE, LPCSTR, LPSTR, SOCK_DGRAM, SOCK_STREAM, UINT, USHORT,
        },
        cvt,
    },
};
use libc::{c_char, c_int, c_ulong, c_void};

pub unsafe extern "system" fn getaddrinfo(
    node: *const c_char,
    service: *const c_char,
    hints: *const ADDRINFOA,
    res: *mut *mut ADDRINFOA,
) -> c_int {
    (*(&load(WSPiApiFunction::GetAddrInfo) as *const *const c_void
        as *const unsafe extern "system" fn(
            node: *const c_char,
            service: *const c_char,
            hints: *const ADDRINFOA,
            res: *mut *mut ADDRINFOA,
        ) -> c_int))(node, service, hints, res)
}

pub unsafe extern "system" fn freeaddrinfo(head: *mut ADDRINFOA) {
    (*(&load(WSPiApiFunction::FreeAddrInfo) as *const *const c_void
        as *const unsafe extern "system" fn(head: *mut ADDRINFOA)))(head)
}

#[derive(Copy, Clone)]
enum WSPiApiFunction {
    GetAddrInfo = 0,
    FreeAddrInfo = 1,
}

#[derive(Copy, Clone)]
#[repr(transparent)]
struct WrappedFn(*const c_void);

unsafe impl Send for WrappedFn {}
unsafe impl Sync for WrappedFn {}

static mut INITIALIZED: bool = false;

static mut RESOLVED_FNS: [WrappedFn; 2] = [WrappedFn(crate::ptr::null()); 2];
const LEGACY_FNS: [WrappedFn; 2] = [
    WrappedFn(wspiapi_legacy_get_addr_info as *const _),
    WrappedFn(wspiapi_legacy_free_addr_info as *const _),
];

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

const AI_PASSIVE: i32 = 0x00000001;
const AI_CANONNAME: i32 = 0x00000002;
const AI_NUMERICHOST: i32 = 0x00000004;

const PF_UNSPEC: i32 = 0;
const PF_INET: i32 = 2;

const SOCK_RAW: i32 = 3;

const INADDR_ANY: u32 = 0x00000000;
const INADDR_LOOPBACK: u32 = 0x7f000001;

const NI_MAXHOST: usize = 1025;

#[inline(always)]
unsafe fn load(function: WSPiApiFunction) -> *const c_void {
    let fn_ptr = RESOLVED_FNS[function as usize].0;

    if fn_ptr.is_null() {
        return api_load(function);
    }

    fn_ptr
}

unsafe fn api_load(function: WSPiApiFunction) -> *const c_void {
    let module = resolve_fns();

    if let Some(fns) = module.and_then(|m| load_fns(m)) {
        RESOLVED_FNS = fns;
    } else {
        RESOLVED_FNS = LEGACY_FNS;
    }

    INITIALIZED = true;

    return RESOLVED_FNS[function as usize].0;
}

#[inline(always)]
unsafe fn resolve_fns() -> Option<HMODULE> {
    let mut system_dir: [u8; (MAX_PATH + 1) as _] = [0; (MAX_PATH + 1) as _];
    let mut path: [u8; (MAX_PATH + 8) as _] = [0; (MAX_PATH + 8) as _];

    cvt(GetSystemDirectoryA(system_dir.as_mut_ptr() as *mut _, MAX_PATH)).ok()?;

    let system_dir_len = CStr::from_ptr(system_dir.as_ptr() as *const _).to_bytes().len() as usize;
    path.copy_from_slice(&system_dir);

    // in Whistler and beyond...
    // the routines are present in the WinSock 2 library (ws2_32.dll).

    path[system_dir_len..system_dir_len + 8].copy_from_slice(b"\\ws2_32\0");

    let library = LoadLibraryA(path.as_ptr() as *const _);

    if !library.is_null() {
        let scratch = GetProcAddress(library, b"getaddrinfo\0".as_ptr() as *const _);

        if scratch.is_null() {
            // doesn't have getaddrinfo
            FreeLibrary(library);
        } else {
            return Some(library);
        }
    }

    // in the IPv6 Technology Preview...
    // the routines are present in the IPv6 WinSock library (wship6.dll).

    path.copy_from_slice(&system_dir);
    path[system_dir_len..system_dir_len + 8].copy_from_slice(b"\\wship6\0");

    let library = LoadLibraryA(path.as_ptr() as *const _);

    if !library.is_null() {
        let scratch = GetProcAddress(library, b"getaddrinfo\0".as_ptr() as *const _);

        if scratch.is_null() {
            // doesn't have getaddrinfo
            FreeLibrary(library);
        } else {
            return Some(library);
        }
    }

    None
}

#[inline(always)]
unsafe fn load_fns(module: HMODULE) -> Option<[WrappedFn; 2]> {
    let mut result = [WrappedFn(crate::ptr::null()); 2];

    const APIS: [&[u8]; 2] = [b"getaddrinfo\0", b"freeaddrinfo\0"];

    for (i, api) in APIS.iter().enumerate() {
        let function = GetProcAddress(module, api.as_ptr() as *const _);

        if function.is_null() {
            FreeLibrary(module);
            return None;
        }

        result[i] = WrappedFn(function);
    }

    Some(result)
}

unsafe extern "system" fn wspiapi_legacy_free_addr_info(mut head: *mut ADDRINFOA) {
    let mut next_ptr = head;

    while !next_ptr.is_null() {
        // scope to make sure the `next` borrow is dropped before freeeing the
        // `ADDRINFOA` it references
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
/// As specified in RFC 2553, Section 6.4.
/// This is the hacked version that only supports IPv4.
///
/// Arguments
///     node              node name to lookup.
///     service           service name to lookup.
///     hints             hints about how to process request.
///     res               where to return result.
///
/// Return Value
///     returns zero if successful, an EAI_* error code if not.
unsafe extern "system" fn wspiapi_legacy_get_addr_info(
    node: *const c_char,
    service: *const c_char,
    hints: *const ADDRINFOA,
    res: *mut *mut ADDRINFOA,
) -> c_int {
    *res = crate::ptr::null_mut();

    // both the node name and the service name can't be NULL.
    if node.is_null() && service.is_null() {
        return EAI_NONAME;
    }

    let mut flags: i32 = 0;
    let mut socket_type: i32 = 0;
    let mut protocol: i32 = 0;

    // validate hints.
    if !hints.is_null() {
        let hints = &*hints;

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
        if flags & AI_CANONNAME == 0 && node.is_null() {
            return EAI_BADFLAGS;
        }

        // we only support a limited number of protocol families.
        if hints.ai_family != PF_UNSPEC && hints.ai_family != PF_INET {
            return EAI_FAMILY;
        }

        // we only support only these socket types.
        socket_type = hints.ai_socktype;
        if socket_type != 0
            && socket_type != SOCK_STREAM
            && socket_type != SOCK_DGRAM
            && socket_type != SOCK_RAW
        {
            return EAI_SOCKTYPE;
        }

        // REVIEW: What if ai_socktype and ai_protocol are at odds?
        protocol = hints.ai_protocol;
    }

    let mut port: USHORT = 0;
    let mut udp_port: USHORT = 0;
    let mut clone: bool = false;

    if !service.is_null() {
        if let Some(raw_port) =
            CStr::from_ptr(service).to_str().ok().and_then(|s| s.parse::<c_ulong>().ok())
        {
            // numeric port string

            port = htons(raw_port as USHORT);
            udp_port = port;
        } else {
            let mut tcp_port: USHORT = 0;

            // non numeric port string

            if socket_type == 0 || socket_type == SOCK_DGRAM {
                let servent = getservbyname(service, b"udp\0".as_ptr() as *const _);
                if !servent.is_null() {
                    port = (*servent).s_port;
                    udp_port = port;
                }
            }

            if socket_type == 0 || socket_type == SOCK_STREAM {
                let servent = getservbyname(service, b"tcp\0".as_ptr() as *const _);
                if !servent.is_null() {
                    port = (*servent).s_port;
                    tcp_port = port;
                }
            }

            // assumes 0 is an invalid service port...
            if port == 0 {
                // no service exists
                return if socket_type == 0 { EAI_NONAME } else { EAI_SERVICE };
            }

            if socket_type == 0 {
                // if both tcp and udp, process tcp now & clone udp later.
                socket_type = if tcp_port != 0 { SOCK_STREAM } else { SOCK_DGRAM };
                clone = tcp_port != 0 && udp_port != 0;
            }
        }
    }

    // do node name lookup...

    // if we weren't given a node name,
    // return the wildcard or loopback address (depending on AI_PASSIVE).
    //
    // if we have a numeric host address string,
    // return the binary address.
    //

    let address: Option<u32> = if node.is_null() {
        Some(htonl(if flags & AI_PASSIVE != 0 { INADDR_ANY } else { INADDR_LOOPBACK }))
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
                (**res).ai_canonname =
                    wspiapi_strdup(CStr::from_ptr(inet_ntoa(in_addr { s_addr: address })));
            }
        }
    } else {
        if flags & AI_NUMERICHOST != 0 {
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
    }

    if error == 0 && clone {
        error = wspiapi_clone(udp_port, *res);
    }

    if error != 0 {
        wspiapi_legacy_free_addr_info(*res);
        *res = crate::ptr::null_mut();
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

/// Routine Description
/// resolve a nodename and return a list of addrinfo structures.
/// IPv4 specific internal function, not exported.
/// *pptResult would need to be freed if an error is returned.
///
/// NOTE: if bAI_CANONNAME is true, the canonical name should be
///       returned in the first addrinfo structure.
///
/// Arguments
/// pszNodeName         name of node to resolve.
/// iSocketType         SOCK_*.  can be wildcarded (zero).
/// iProtocol           IPPROTO_*.  can be wildcarded (zero).
/// wPort               port number of service (in network order).
/// bAI_CANONNAME       whether the AI_CANONNAME flag is set.
/// pptResult           where to return result.
///
/// Return Value
/// Returns 0 on success, an EAI_* style error value otherwise.
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

        if error > 0 {
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
        (**res).ai_canonname = wspiapi_strdup(CStr::from_ptr(alias_ref.as_ptr() as *const _));
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
    if !host.is_null() {
        let host = &*host;

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

                next = &mut (**next).ai_next as *mut *mut _;

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
        ai_canonname: crate::ptr::null_mut(),
        ai_flags: 0,
        ai_next: crate::ptr::null_mut(),
    };

    Box::into_raw(new)
}

unsafe fn wspiapi_parse_v4_address(address: &CStr) -> Option<u32> {
    if address.to_bytes().iter().filter(|&&c| c == b'.').count() != 3 {
        return None;
    }

    let addr: u32 = inet_addr(address.as_ptr());

    const INADDR_NONE: u32 = 0xffffffff;
    if addr == INADDR_NONE {
        return None;
    }

    return Some(addr);
}

#[inline]
fn wspiapi_strdup(string: &CStr) -> *mut c_char {
    string.to_owned().into_raw()
}

// from Winsock2.h
#[repr(C)]
struct servent {
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
struct hostent {
    h_name: *const c_char,
    h_aliases: *const *const c_char,
    h_addrtype: USHORT,
    h_length: USHORT,
    h_addr_list: *const *const c_char,
}

const MAX_PATH: UINT = 260;

type FARPROC = *mut c_void;
extern "system" {
    fn GetSystemDirectoryA(lpBuffer: LPSTR, uSize: UINT) -> UINT;
    fn LoadLibraryA(lpFileName: LPCSTR) -> HMODULE;
    fn FreeLibrary(hLibModule: HMODULE) -> BOOL;
    fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;

    fn htons(hostshort: USHORT) -> USHORT;
    fn htonl(hostshort: u32) -> u32;

    /// The pointer that is returned points to the SERVENT structure allocated by the
    /// Windows Sockets library. The application must never attempt to modify this
    /// structure or to free any of its components. Furthermore only one copy of this
    /// structure is allocated per thread, so the application should copy any information
    /// it needs before issuing any other Windows Sockets function calls.
    fn getservbyname(name: *const c_char, proto: *const c_char) -> *const servent;
    /// The `gethostbyname` function returns a pointer to a hostent structure—a structure allocated
    /// by Windows Sockets. The hostent structure contains the results of a successful search for
    /// the host specified in the name parameter.

    /// The application must never attempt to modify this structure or to free any of its
    /// components. Furthermore, only one copy of this structure is allocated per thread, so the
    /// application should copy any information it needs before issuing any other Windows Sockets
    /// function calls.
    fn gethostbyname(name: *const c_char) -> *const hostent;
    fn inet_addr(cp: *const c_char) -> u32;
    fn inet_ntoa(r#in: in_addr) -> *const c_char;
}
