use crate::spec::{LinkerFlavor, LldFlavor, Target, TargetResult};

pub fn target() -> TargetResult {
    let mut base = super::windows_msvc_base::opts();
    base.cpu = "pentium".to_string();
    base.max_atomic_width = Some(64);
    base.target_api_default_features = vec!["4.10.2222".to_string()];

    let pre_link_args_msvc = vec![
        // Disable SAFESEH since VC6 libraries don't support it
        "/SAFESEH:NO".to_string(),
        // Link to ___CxxFrameHandler (XP and earlier MSVCRT) instead of ___CxxFrameHandler3.
        // This cannot be done in the MSVC `eh_personality `handling because LLVM hardcodes SEH
        // support based on that name sadly
        "/ALTERNATENAME:___CxxFrameHandler3=___CxxFrameHandler".to_string(),
    ];
    base.pre_link_args.get_mut(&LinkerFlavor::Msvc).unwrap().extend(pre_link_args_msvc.clone());
    base.pre_link_args
        .get_mut(&LinkerFlavor::Lld(LldFlavor::Link))
        .unwrap()
        .extend(pre_link_args_msvc);

    Ok(Target {
        llvm_target: "i586-pc-windows-msvc".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "32".to_string(),
        target_c_int_width: "32".to_string(),
        data_layout: "e-m:x-p:32:32-p270:32:32-p271:32:32-p272:64:64-\
            i64:64-f80:32-n8:16:32-a:0:32-S32"
            .to_string(),
        arch: "x86".to_string(),
        target_os: "windows".to_string(),
        target_env: "msvc".to_string(),
        target_vendor: "ansi".to_string(),
        linker_flavor: LinkerFlavor::Msvc,
        options: base,
    })
}
