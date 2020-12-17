//! See <https://0x.org/docs/guides/0x-cheat-sheet>

pub enum Chain {
    Mainnet,
    Kovan,
    Ropsten,
    Rinkeby,
    GanacheSnapshot,
}

pub enum ProtocolVersion {
    V1,
    V2,
}
