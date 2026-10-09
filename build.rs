fn main() {
    println!("cargo:rerun-if-changed=shaders/voxel.wgsl");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=data/content.json");
    println!("cargo:rerun-if-changed=data/config.json");
    println!("cargo:rerun-if-changed=data/terrain.json");
    let mut paths = std::fs::read_dir("src")
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    paths.extend(
        [
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "data/content.json",
            "data/config.json",
            "data/terrain.json",
            "shaders/voxel.wgsl",
        ]
        .map(std::path::PathBuf::from),
    );
    paths.sort();
    let mut hash = 0xcbf29ce484222325u64;
    for path in paths {
        for b in std::fs::read(path).unwrap() {
            hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rustc-env=HV_SOURCE_HASH={hash:016x}");
    let source = std::fs::read_to_string("shaders/voxel.wgsl").unwrap();
    let module = naga::front::wgsl::parse_str(&source).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
    for (name, stage) in [
        ("vs", naga::ShaderStage::Vertex),
        ("fs", naga::ShaderStage::Fragment),
    ] {
        let words = naga::back::spv::write_vec(
            &module,
            &info,
            &naga::back::spv::Options {
                // CPU projection already produces Vulkan clip coordinates.
                flags: naga::back::spv::WriterFlags::empty(),
                ..Default::default()
            },
            Some(&naga::back::spv::PipelineOptions {
                shader_stage: stage,
                entry_point: name.into(),
            }),
        )
        .unwrap();
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        std::fs::write(
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(format!("{name}.spv")),
            bytes,
        )
        .unwrap();
    }
}
