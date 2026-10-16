fn main() {
    embed_resource::compile("RustyClicker.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed the RustyClicker Windows icon");
}
