fn main() {
    // Embed the absolute path to the MCP server script at build time for development.
    let mcp_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .join("mcp");
    println!("cargo:rustc-env=IRE_MCP_DIR={}", mcp_dir.display());
    tauri_build::build()
}
