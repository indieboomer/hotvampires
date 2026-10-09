fn main() {
    println!("cargo:rerun-if-changed=shaders/voxel.wgsl");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=data/content.json");
    println!("cargo:rerun-if-changed=data/config.json");
    println!("cargo:rerun-if-changed=data/terrain.json");
    println!("cargo:rerun-if-changed=data/voxel.json");
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
            "data/voxel.json",
            "shaders/voxel.wgsl",
            "data/voxel-textures.json",
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
    println!("cargo:rerun-if-changed=data/voxel-textures.json");
    let tiles: serde_json::Value =
        serde_json::from_slice(&std::fs::read("data/voxel-textures.json").unwrap()).unwrap();
    let mut rows = Vec::new();
    for tile in tiles["tiles"].as_array().unwrap() {
        let pixels = tile["pixels"].as_array().unwrap();
        assert_eq!(pixels.len(), 64);
        for row in pixels.chunks_exact(8) {
            let packed = row.iter().enumerate().fold(0u32, |bits, (x, p)| {
                let value = p.as_u64().unwrap();
                assert!(value < 16);
                bits | (value as u32) << (x * 4)
            });
            rows.push(format!("{packed}u"));
        }
    }
    assert_eq!(rows.len(), 64);
    let source = format!(
        "const TEXTURE_ROWS: array<u32,64> = array<u32,64>({});\n{}",
        rows.join(","),
        std::fs::read_to_string("shaders/voxel.wgsl").unwrap()
    );
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
                // The vertex shader already emits Vulkan clip coordinates.
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
