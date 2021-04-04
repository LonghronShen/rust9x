use crate::spec::{LinkerFlavor, LldFlavor, Target};

pub fn target() -> Target {
    let mut base = super::windows_msvc_base::opts();
    base.cpu = "pentium4".to_string();
    base.max_atomic_width = Some(64);
    base.vendor = "oldpc".to_string();

    let pre_link_args_msvc = vec![
        // Link to ___CxxFrameHandler (XP and earlier MSVCRT) instead of ___CxxFrameHandler3.
        // This cannot be done in the MSVC `eh_personality` handling because LLVM hardcodes SEH
        // support based on that name, sadly
        "/ALTERNATENAME:___CxxFrameHandler3=___CxxFrameHandler".to_string(),
    ];
    base.pre_link_args.entry(LinkerFlavor::Msvc).or_default().extend(pre_link_args_msvc.clone());
    base.pre_link_args
        .entry(LinkerFlavor::Lld(LldFlavor::Link))
        .or_default()
        .extend(pre_link_args_msvc);

    Target {
        llvm_target: "i686-pc-windows-msvc".to_string(),
        pointer_width: 32,
        data_layout: "e-m:x-p:32:32-p270:32:32-p271:32:32-p272:64:64-\
            i64:64-f80:32-n8:16:32-a:0:32-S32"
            .to_string(),
        arch: "x86".to_string(),
        options: base,
    }
}
