fn main() {
    uniffi::generate_scaffolding("src/bridge_core.udl").expect("generate UniFFI scaffolding");
}
