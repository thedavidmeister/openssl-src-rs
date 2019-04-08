extern crate cc;

use std::env;
use std::fs;
use std::path::{PathBuf, Path};
use std::process::Command;

pub fn source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("openssl")
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub struct Build {
    out_dir: Option<PathBuf>,
    target: Option<String>,
    host: Option<String>,
}

pub struct Artifacts {
    include_dir: PathBuf,
    lib_dir: PathBuf,
    libs: Vec<String>,
}

impl Build {
    pub fn new() -> Build {
        Build {
            out_dir: env::var_os("OUT_DIR").map(|s| {
                PathBuf::from(s).join("openssl-build")
            }),
            target: env::var("TARGET").ok(),
            host: env::var("HOST").ok(),
        }
    }

    pub fn out_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Build {
        self.out_dir = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn target(&mut self, target: &str) -> &mut Build {
        self.target = Some(target.to_string());
        self
    }

    pub fn host(&mut self, host: &str) -> &mut Build {
        self.host = Some(host.to_string());
        self
    }

    fn cmd_make(&self) -> Command {
        match &self.host.as_ref().expect("HOST dir not set")[..] {
            "x86_64-unknown-dragonfly" => Command::new("gmake"),
            "x86_64-unknown-freebsd" => Command::new("gmake"),
            _ => Command::new("make"),
        }
    }

    pub fn build(&mut self) -> Artifacts {
        println!("fooo build");

        let target = &self.target.as_ref().expect("TARGET dir not set")[..];
        let host = &self.host.as_ref().expect("HOST dir not set")[..];
        let out_dir = self.out_dir.as_ref().expect("OUT_DIR not set");
        let build_dir = out_dir.join("build");
        let install_dir = out_dir.join("install");

        println!("xxx");
        println!("x build_dir: {:?}", build_dir);
        println!("x install_dir: {:?}", install_dir);
        if build_dir.exists() {
            fs::remove_dir_all(&build_dir).unwrap();
        }
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).unwrap();
        }

        println!("yyy");
        let inner_dir = build_dir.join("src");
        println!("inner_dir: {:?}", inner_dir);
        fs::create_dir_all(&inner_dir).unwrap();
        println!("source_dir: {:?}", source_dir());
        cp_r(&source_dir(), &inner_dir);

        println!("zzz");
        let mut configure = Command::new("perl");
        configure.arg("./Configure");
        if target.contains("pc-windows-gnu") {
            configure.arg(&format!("--prefix={}", sanitize_sh(&install_dir)));
        } else {
            configure.arg(&format!("--prefix={}", install_dir.display()));
        }

        println!("aaa");

        configure
            // No shared objects, we just want static libraries
            .arg("no-dso")

            // Should be off by default on OpenSSL 1.1.0, but let's be extra sure
            .arg("no-ssl3")

            // No need to build tests, we won't run them anyway
            .arg("no-unit-test")

            // Nothing related to zlib please
            .arg("no-comp")
            .arg("no-zlib")
            .arg("no-zlib-dynamic")

            // This actually fails to compile on musl (it needs linux/version.h
            // right now) but we don't actually need this most of the time. This
            // is intended for super-configurable backends and whatnot
            // apparently but the whole point of this script is to produce a
            // "portable" implementation of OpenSSL, so shouldn't be any harm in
            // turning this off.
            .arg("no-engine")

            // MUSL doesn't implement some of the libc functions that the async
            // stuff depends on, and we don't bind to any of that in any case.
            .arg("no-async");

        // On Android it looks like not passing no-stdio may cause a build
        // failure (#13), but most other platforms need it for things like
        // loading system certificates so only disable it on Android.
        if target.contains("android") {
            configure.arg("no-stdio");
        }

        if target.contains("msvc") {
            // On MSVC we need nasm.exe to compile the assembly files, but let's
            // just pessimistically assume for now that's not available.
            configure.arg("no-asm");

            let features = env::var("CARGO_CFG_TARGET_FEATURE")
                      .unwrap_or(String::new());
            if features.contains("crt-static") {
                configure.arg("no-shared");
            }
        } else {
            // Never shared on non-MSVC
            configure.arg("no-shared");
        }

        let os = match target {
            // Note that this, and all other android targets, aren't using the
            // `android64-aarch64` (or equivalent) builtin target. That
            // apparently has a crazy amount of build logic in OpenSSL 1.1.1
            // that bypasses basically everything `cc` does, so let's just cop
            // out and say it's linux and hope it works.
            "aarch64-linux-android" => "linux-aarch64",
            "aarch64-unknown-linux-gnu" => "linux-aarch64",
            "arm-linux-androideabi" => "linux-armv4",
            "armv7-linux-androideabi" => "linux-armv4",
            "arm-unknown-linux-gnueabi" => "linux-armv4",
            "arm-unknown-linux-gnueabihf" => "linux-armv4",
            "armv7-unknown-linux-gnueabihf" => "linux-armv4",
            "armv7-unknown-linux-musleabihf" => "linux-armv4",
            "asmjs-unknown-emscripten" => "gcc",
            "i686-apple-darwin" => "darwin-i386-cc",
            "i686-linux-android" => "linux-elf",
            "i686-pc-windows-gnu" => "mingw",
            "i686-pc-windows-msvc" => "VC-WIN32",
            "i686-unknown-freebsd" => "BSD-x86-elf",
            "i686-unknown-linux-gnu" => "linux-elf",
            "i686-unknown-linux-musl" => "linux-elf",
            "mips-unknown-linux-gnu" => "linux-mips32",
            "mips64-unknown-linux-gnuabi64" => "linux64-mips64",
            "mips64el-unknown-linux-gnuabi64" => "linux64-mips64",
            "mipsel-unknown-linux-gnu" => "linux-mips32",
            "powerpc-unknown-linux-gnu" => "linux-ppc",
            "powerpc64-unknown-linux-gnu" => "linux-ppc64",
            "powerpc64le-unknown-linux-gnu" => "linux-ppc64le",
            "s390x-unknown-linux-gnu" => "linux64-s390x",
            "x86_64-apple-darwin" => "darwin64-x86_64-cc",
            "x86_64-linux-android" => "linux-x86_64",
            "x86_64-pc-windows-gnu" => "mingw64",
            "x86_64-pc-windows-msvc" => "VC-WIN64A",
            "x86_64-unknown-freebsd" => "BSD-x86_64",
            "x86_64-unknown-dragonfly" => "BSD-x86_64",
            "x86_64-unknown-linux-gnu" => "linux-x86_64",
            "x86_64-unknown-linux-musl" => "linux-x86_64",
            "x86_64-unknown-netbsd" => "BSD-x86_64",
            "wasm32-unknown-emscripten" => "gcc",
            "wasm32-unknown-unknown" => "gcc",
            _ => panic!("don't know how to configure OpenSSL for {}", target),
        };

        println!("**");

        configure.arg(os);

        // If we're not on MSVC we configure cross compilers and cross tools and
        // whatnot. Note that this doesn't happen on MSVC b/c things are pretty
        // different there and this isn't needed most of the time anyway.
        if !target.contains("msvc") {
            let mut cc = cc::Build::new();
            cc.target(target)
                .host(host)
                .warnings(false)
                .opt_level(2);
            let compiler = cc.get_compiler();
            configure.env("CC", compiler.path());
            let path = compiler.path().to_str().unwrap();

            // Infer ar/ranlib tools from cross compilers if the it looks like
            // we're doing something like `foo-gcc` route that to `foo-ranlib`
            // as well.
            if path.ends_with("-gcc") && !target.contains("unknown-linux-musl") {
                let path = &path[..path.len() - 4];
                configure.env("RANLIB", format!("{}-ranlib", path));
                configure.env("AR", format!("{}-ar", path));
            }

            // Make sure we pass extra flags like `-ffunction-sections` and
            // other things like ARM codegen flags.
            for arg in compiler.args() {
                // For whatever reason `-static` on MUSL seems to cause
                // issues...
                if target.contains("musl") && arg == "-static" {
                    continue
                }
                configure.arg(arg);
            }

            if target == "x86_64-pc-windows-gnu" {
                // For whatever reason OpenSSL 1.1.1 fails to build on
                // `x86_64-pc-windows-gnu` in our docker container due to an
                // error about "too many sections". Having no idea what this
                // error is about some quick googling yields
                // https://github.com/cginternals/glbinding/issues/135 which
                // mysteriously mentions `-Wa,-mbig-obj`, passing a new argument
                // to the assembler. Now I have no idea what `-mbig-obj` does
                // for Windows nor why it would matter, but it does seem to fix
                // compilation issues.
                //
                // Note that another entirely unrelated issue -
                // https://github.com/assimp/assimp/issues/177 - was fixed by
                // splitting a large file, so presumably OpenSSL has a large
                // file soemwhere in it? Who knows!
                configure.arg("-Wa,-mbig-obj");
            }

            if target.contains("pc-windows-gnu") && path.ends_with("-gcc") {
                // As of OpenSSL 1.1.1 the build system is now trying to execute
                // `windres` which doesn't exist when we're cross compiling from
                // Linux, so we may need to instruct it manually to know what
                // executable to run.
                let windres = format!("{}-windres", &path[..path.len() - 4]);
                configure.env("WINDRES", &windres);
            }

            if target.contains("emscripten") {
                // As of OpenSSL 1.1.1 the source apparently wants to include
                // `stdatomic.h`, but this doesn't exist on Emscripten. After
                // reading OpenSSL's source where the error is, we define this
                // magical (and probably
                // compiler-internal-should-not-be-user-defined) macro to say
                // "no atomics are available" and avoid including such a header.
                configure.arg("-D__STDC_NO_ATOMICS__");
            }

            if target.contains("musl") {
                // Hack around openssl/openssl#7207 for now
                configure.arg("-DOPENSSL_NO_SECURE_MEMORY");
            }
        }

        // And finally, run the perl configure script!
        configure.current_dir(&inner_dir);
        println!("target debug: {:?}", target);
        println!("host debug: {:?}", host);
        println!("out_dir: {:?}", out_dir);
        println!("build_dir: {:?}", build_dir);
        println!("install_dir: {:?}", install_dir);
        println!("os: {:?}", os);
        println!("configure debug: {:?}", configure);
        self.run_command(configure, "configuring OpenSSL build");

        // On MSVC we use `nmake.exe` with a slightly different invocation, so
        // have that take a different path than the standard `make` below.
        if target.contains("msvc") {
            let mut build = cc::windows_registry::find(target, "nmake.exe")
                .expect("failed to find nmake");
            build.current_dir(&inner_dir);
            self.run_command(build, "building OpenSSL");

            let mut install = cc::windows_registry::find(target, "nmake.exe")
                .expect("failed to find nmake");
            install.arg("install_sw").current_dir(&inner_dir);
            self.run_command(install, "installing OpenSSL");
        } else {
            let mut depend = self.cmd_make();
            depend.arg("depend").current_dir(&inner_dir);
            self.run_command(depend, "building OpenSSL dependencies");

            let mut build = self.cmd_make();
            build.current_dir(&inner_dir);
            if !cfg!(windows) {
                if let Some(s) = env::var_os("CARGO_MAKEFLAGS") {
                    build.env("MAKEFLAGS", s);
                }
            }
            self.run_command(build, "building OpenSSL");

            let mut install = self.cmd_make();
            install.arg("install_sw").current_dir(&inner_dir);
            self.run_command(install, "installing OpenSSL");
        }

        let libs = if target.contains("msvc") {
            vec!["libssl".to_string(), "libcrypto".to_string()]
        } else {
            vec!["ssl".to_string(), "crypto".to_string()]
        };

        fs::remove_dir_all(&inner_dir).unwrap();

        Artifacts {
            lib_dir: install_dir.join("lib"),
            include_dir: install_dir.join("include"),
            libs: libs,
        }
    }

    fn run_command(&self, mut command: Command, desc: &str) {
        println!("running {:?}", command);
        let status = command.status().unwrap();
        if !status.success() {
            panic!("


Error {}:
    Command: {:?}
    Exit status: {}


    ",
                desc,
                command,
                status);
        }
    }
}

fn cp_r(src: &Path, dst: &Path) {
    for f in fs::read_dir(src).unwrap() {
        let f = f.unwrap();
        let path = f.path();
        let name = path.file_name().unwrap();
        let dst = dst.join(name);
        if f.file_type().unwrap().is_dir() {
            fs::create_dir_all(&dst).unwrap();
            cp_r(&path, &dst);
        } else {
            let _ = fs::remove_file(&dst);
            fs::copy(&path, &dst).unwrap();
        }
    }
}

fn sanitize_sh(path: &Path) -> String {
    if !cfg!(windows) {
        return path.to_str().unwrap().to_string()
    }
    let path = path.to_str().unwrap().replace("\\", "/");
    return change_drive(&path).unwrap_or(path);

    fn change_drive(s: &str) -> Option<String> {
        let mut ch = s.chars();
        let drive = ch.next().unwrap_or('C');
        if ch.next() != Some(':') {
            return None
        }
        if ch.next() != Some('/') {
            return None
        }
        Some(format!("/{}/{}", drive, &s[drive.len_utf8() + 2..]))
    }
}

impl Artifacts {
    pub fn include_dir(&self) -> &Path {
        &self.include_dir
    }

    pub fn lib_dir(&self) -> &Path {
        &self.lib_dir
    }

    pub fn libs(&self) -> &[String] {
        &self.libs
    }

    pub fn print_cargo_metadata(&self) {
        println!("cargo:rustc-link-search=native={}", self.lib_dir.display());
        for lib in self.libs.iter() {
            println!("cargo:rustc-link-lib=static={}", lib);
        }
        println!("cargo:include={}", self.include_dir.display());
        println!("cargo:lib={}", self.lib_dir.display());
    }
}
