//! JVM access flags the checker reads (`ACC_*` of the class file format).

pub const PUBLIC: u16 = 0x0001;
pub const PRIVATE: u16 = 0x0002;
pub const PROTECTED: u16 = 0x0004;
pub const STATIC: u16 = 0x0008;
pub const FINAL: u16 = 0x0010;
pub const INTERFACE: u16 = 0x0200;
pub const ABSTRACT: u16 = 0x0400;
pub const ENUM: u16 = 0x4000;
