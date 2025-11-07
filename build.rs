fn main() {
    let mut build = cc::Build::new();
    
    if let Ok(ldflags) = std::env::var("CFLAGS") {
        for flag in ldflags.split_whitespace() {
            if flag.starts_with("-L") {
                println!("cargo:rustc-link-search=native={}", &flag[2..]);
            } else if flag.starts_with("-l") {
                println!("cargo:rustc-link-lib={}", &flag[2..]);
            }
        }
    }
}
