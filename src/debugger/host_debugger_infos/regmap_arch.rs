use super::regmap_arch_amd64::Amd64NativeRegisterInfo;

pub struct DummyNativeRegisterInfo {}

#[cfg(target_arch = "x86_64")]
pub type ArchNativeRegisterInfo = Amd64NativeRegisterInfo;

#[cfg(not(target_arch = "x86_64"))]
pub type ArchNativeRegisterInfo = DummyNativeRegisterInfo;
