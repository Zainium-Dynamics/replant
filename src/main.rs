//! replant: moves hard-coded FHS paths in a source tree to the Zainium OS layout.

use std::process::ExitCode;

/// Old path prefix and where it lives on Zainium OS.
const MAP: &[(&str, &str)] = &[
    ("/usr/bin", "/overlayer/syshub/bin"),
    ("/usr/sbin", "/overlayer/syshub/sbin"),
    ("/usr/lib", "/overlayer/syshub/lib"),
    ("/usr/share", "/overlayer/syshub/share"),
    ("/usr/include", "/overlayer/syshub/include"),
    ("/lib/firmware", "/overlayer/syshub/drivers/hardware/firmwares"),
    ("/lib64", "/overlayer/syshub/lib"),
    ("/lib", "/overlayer/syshub/lib"),
    ("/bin", "/overlayer/syshub/bin"),
    ("/sbin", "/overlayer/syshub/sbin"),
    ("/etc", "/overlayer/syshub/etc"),
    ("/var", "/overlayer/syshub/var"),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("map") => {
            for (from, to) in MAP {
                println!("{from:<14} {to}");
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: replant map");
            eprintln!("       replant <source-dir>   (not written yet)");
            ExitCode::from(2)
        }
    }
}
