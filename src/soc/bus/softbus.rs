//! DeviceBus owns the SoC physical memory map, handling device registration, hashed lookups,
//! and prioritised overlays so consumers get deterministic address-to-device resolution
//! without mutating shared state. It mirrors the .NET BasicHashedDeviceBus logic while
//! providing Rust-friendly error handling and concurrency semantics.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::soc::{bus::DeviceHandle, device::Device};

use super::{
    error::{BusError, BusResult},
    range::BusRange,
};

const DEVICE_PRIORITY: u8 = 0;

pub type DeviceRef = Arc<Mutex<dyn Device>>;

///Implement the device bus, owning device registrations and address mappings
pub struct DeviceBus {
    // Linear list of devices, allowing O(1) access by ID
    devices: Vec<DeviceRef>,
    // Mapping physical address ranges to Device IDs
    // Key: Start Address -> (End Address, DeviceId, RemapOffset)
    map: BTreeMap<usize, BusRange>,
}

impl DeviceBus {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            map: BTreeMap::new(),
        }
    }

    pub fn map_device(
        &mut self,
        device: impl Device + 'static,
        address: usize,
        priority: u8,
    ) -> BusResult<()> {
        // Insert into 'devices' then update 'map'.
        // To handle priority: If ranges overlap, higher priority overrides /splits lower priority ranges.
        let device_range = device.span();
        self.devices.push(Arc::new(Mutex::new(device)));
        let device_id = self.devices.len() - 1;
        let range = BusRange {
            bus_start: address,
            bus_end: address + device_range.len(),
            device_offset: device_range.start,
            device_id,
            priority,
        };
        self.insert_range(range)
    }

    pub fn resolve(&self, address: usize) -> BusResult<DeviceHandle> {
        let range = self
            .range_for_address(address)
            .ok_or(BusError::InvalidAddress { address })?;
        Ok(DeviceHandle::new(
            self.devices[range.device_id].clone(),
            (address - range.bus_start) + range.device_offset,
        ))
    }

    pub fn unmap(&mut self, address: usize) -> BusResult<()> {
        let key = self
            .range_key_for_address(address)
            .ok_or(BusError::NotMapped { address })?;
        self.map.remove(&key);
        Ok(())
    }

    /// Returns the physical device reference and cloned range that contains `address`.
    /// Callers that need to construct custom views (TLBs, MMUs, etc) can reuse this to
    /// validate that a downstream redirect stays within a single device span.
    pub fn resolve_device_at(&self, address: usize) -> BusResult<(DeviceRef, BusRange)> {
        let range = self
            .range_for_address(address)
            .cloned()
            .ok_or(BusError::InvalidAddress { address })?;
        Ok((self.devices[range.device_id].clone(), range))
    }
}

impl DeviceBus {
    fn insert_range(&mut self, range: BusRange) -> BusResult<()> {
        self.clear_overlaps(&range)?;
        self.map.insert(range.bus_start, range);
        Ok(())
    }

    fn range_for_address(&self, address: usize) -> Option<&BusRange> {
        self.map
            .range(..=address)
            .next_back()
            .and_then(|(_, range)| {
                if address < range.bus_end {
                    Some(range)
                } else {
                    None
                }
            })
    }

    fn range_key_for_address(&self, address: usize) -> Option<usize> {
        self.map
            .range(..=address)
            .next_back()
            .and_then(|(start, range)| {
                if address < range.bus_end {
                    Some(*start)
                } else {
                    None
                }
            })
    }

    fn clear_overlaps(&mut self, range: &BusRange) -> BusResult<()> {
        let keys = self.collect_overlap_keys(range.bus_start, range.bus_end);
        let mut reinserts = Vec::new();

        for key in keys {
            if let Some(existing) = self.map.remove(&key) {
                if existing.bus_end <= range.bus_start || existing.bus_start >= range.bus_end {
                    reinserts.push(existing);
                    continue;
                }

                if existing.priority >= range.priority {
                    reinserts.push(existing);
                    for segment in reinserts {
                        self.map.insert(segment.bus_start, segment);
                    }
                    return Err(BusError::Overlap {
                        address: range.bus_start,
                        details: "higher priority mapping already present".into(),
                    });
                }

                if existing.bus_start < range.bus_start {
                    reinserts.push(self.slice_range(
                        &existing,
                        existing.bus_start,
                        range.bus_start,
                    ));
                }

                if existing.bus_end > range.bus_end {
                    reinserts.push(self.slice_range(&existing, range.bus_end, existing.bus_end));
                }
            }
        }

        for segment in reinserts {
            self.map.insert(segment.bus_start, segment);
        }

        Ok(())
    }

    fn collect_overlap_keys(&self, start: usize, end: usize) -> Vec<usize> {
        let mut keys = Vec::new();
        if let Some((&key, range)) = self.map.range(..=start).next_back() {
            if range.bus_end > start {
                keys.push(key);
            }
        }
        for (&key, _) in self.map.range(start..end) {
            keys.push(key);
        }
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    fn slice_range(&mut self, source: &BusRange, start: usize, end: usize) -> BusRange {
        let mut segment = source.clone();
        segment.bus_start = start;
        segment.bus_end = end;
        segment.device_offset = source.device_offset + (start - source.bus_start);
        segment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{AccessContext, DeviceResult, Endianness};
    use std::ops::Range;

    struct ProbeDevice {
        name: String,
        backing: Vec<u8>,
    }

    impl ProbeDevice {
        fn new(name: &str, len: usize) -> Self {
            Self::with_fill(name, len, 0)
        }

        fn with_fill(name: &str, len: usize, fill: u8) -> Self {
            Self {
                name: name.to_string(),
                backing: vec![fill; len],
            }
        }
    }

    impl Device for ProbeDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn span(&self) -> Range<usize> {
            0..self.backing.len()
        }

        fn endianness(&self) -> Endianness {
            Endianness::Little
        }

        fn read(&mut self, _offset: usize, _out: &mut [u8], _ctx: AccessContext) -> DeviceResult<()> {
            Ok(())
        }

        fn write(&mut self, _offset: usize, _data: &[u8], _ctx: AccessContext) -> DeviceResult<()> {
            Ok(())
        }
    }

    #[test]
    fn register_device_and_resolve_returns_expected_mapping() {
        let mut bus = DeviceBus::new();
        let probe = ProbeDevice::new("probe", 0x2000);
        bus.map_device(probe, 0x4000, DEVICE_PRIORITY)
            .expect("register device");

        let handle = bus.resolve(0x4000).expect("resolve mapped address");
        assert_eq!(
            handle.get_device_name(),
            "probe",
            "resolved handle should map to registered device"
        );

        let verifier = bus.resolve(0x5000).expect("resolve for verification");
        assert_eq!(
            verifier.get_device_name(),
            "probe",
            "resolved handle should map to registered device"
        );

        assert!(bus.resolve(0x6000).is_err(), "unmapped address should error");
    }

    #[test]
    fn redirect_range_aliases_target_bytes() {
        let mut bus = DeviceBus::new();
        let probe = ProbeDevice::new("probe", 0x3000);
        bus.map_device(probe, 0x4000, DEVICE_PRIORITY)
            .expect("register device");

        bus.map_range(0x2000, 0x20, 0x4100, REDIRECT_PRIORITY)
            .expect("map redirect");

        let alias = bus.resolve(0x2000).expect("resolve alias");
        let source = bus.resolve(0x4100).expect("resolve source");

        assert_eq!(
            alias.get_device_name(),
            source.get_device_name(),
            "redirected handle should map to same device"
        );
        assert_eq!(
            alias.get_position(),
            source.get_position() - 0x100,
            "redirected handle position should match offset from target"
        );
    }

    #[test]
    fn lower_priority_blocked_until_higher_removed() {
        let mut bus = DeviceBus::new();
        let high = ProbeDevice::with_fill("hi", 0x100, 0xAA);
        bus.map_device(high, 0x8000, DEVICE_PRIORITY + 5)
            .expect("register high priority device");

        let low = ProbeDevice::with_fill("lo", 0x100, 0x33);
        bus.map_device(low, 0x8000, DEVICE_PRIORITY).expect("mapping succeeds");

        let handle = bus.resolve(0x8000).expect("resolve address");
        assert_eq!(
            handle.get_device_name(),
            "hi",
            "higher priority device should take precedence"
        );
        bus.unmap(0x8000).expect("remove higher priority range");
        let handle = bus.resolve(0x8000).expect("resolve address");
        assert_eq!(
            handle.get_device_name(),
            "lo",
            "lower priority device should now be visible"
        );
    }

    #[test]
    fn higher_priority_creates_hole_in_lower_range() {
        let mut bus = DeviceBus::new();
        let low = ProbeDevice::with_fill("low", 0x200, 0x11);
        bus.map_device(low, 0x2000, DEVICE_PRIORITY)
            .expect("register low priority range");

        let high = ProbeDevice::with_fill("high", 0x40, 0xEE);
        bus.map_device(high, 0x2060, DEVICE_PRIORITY + 10)
            .expect("register high priority slice");

        let handle_low = bus.resolve(0x2000).expect("resolve low start");
        assert_eq!(
            handle_low.get_device_name(),
            "low",
            "low priority device should be visible before high range"
        );
        let handle_high = bus.resolve(0x2060).expect("resolve high start");
        assert_eq!(
            handle_high.get_device_name(),
            "high",
            "high priority device should be visible in its range"
        );
        let handle_low2 = bus.resolve(0x20A0).expect("resolve low after high range");
        assert_eq!(
            handle_low2.get_device_name(),
            "low",
            "low priority device should be visible after high range"
        );
    }
}
