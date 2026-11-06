fn main() {
    println!("cargo:rerun-if-changed=windows-deck.rc");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    embed_resource::compile("windows-deck.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("cannot embed the exe icon");
}
