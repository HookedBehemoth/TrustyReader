fn main() {
    // make sure linkall.x is the last linker script (otherwise might cause problems with flip-link)
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    cc::Build::new()
        .compiler("xtensa-esp32s3-elf-gcc")
        .file("../libs/fatfs/ff.c")
        .file("../libs/fatfs/ffsystem.c")
        .file("../libs/fatfs/ffunicode.c")
        .file("../libs/fatfs/compat.c")
        .flag("-mlongcalls")
        .compile("fatfs");
    println!("cargo:rerun-if-changed=../libs/fatfs/diskio.h");
    println!("cargo:rerun-if-changed=../libs/fatfs/ff.c");
    println!("cargo:rerun-if-changed=../libs/fatfs/ff.h");
    println!("cargo:rerun-if-changed=../libs/fatfs/ffconf.h");
    println!("cargo:rerun-if-changed=../libs/fatfs/ffsystem.c");
    println!("cargo:rerun-if-changed=../libs/fatfs/ffunicode.c");
    println!("cargo:rerun-if-changed=../libs/fatfs/compat.c");
}
