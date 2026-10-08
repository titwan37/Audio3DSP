// Audio3DSP — DSP Module Root
//
// Re-exports all DSP components and the chain orchestrator.

pub mod chain;
pub mod crossfeed;
pub mod dynamics;
pub mod emboss;
pub mod haas;
pub mod master_strip;
pub mod reverb;
pub mod saturation;
pub mod utility;
pub mod venue_expander;
pub mod widener;

#[allow(unused_imports)]
pub use chain::DspChain;
#[allow(unused_imports)]
pub use crossfeed::HeadphoneCrossfeed;
#[allow(unused_imports)]
pub use dynamics::StereoCompressor;
#[allow(unused_imports)]
pub use master_strip::MasterStrip;
#[allow(unused_imports)]
pub use saturation::ConsoleSaturation;
#[allow(unused_imports)]
pub use utility::Utility;
#[allow(unused_imports)]
pub use venue_expander::VenueExpander;
#[allow(unused_imports)]
pub use emboss::VocalEmboss;
#[allow(unused_imports)]
pub use widener::MsWidener;
