//! DeviceBus owns the SoC memory map, handling device registration, hashed lookups,
//! and redirect overlays so consumers get deterministic address-to-device resolution
//! without mutating shared state. It mirrors the .NET BasicHashedDeviceBus logic while
//! providing Rust-friendly error handling and concurrency semantics.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::soc::{bus::DeviceHandle, device::Device};

use super::{
    error::{BusError, BusResult},
    range::{BusRange, RangeKind},
};

const DEVICE_PRIORITY: u8 = 0;
const REDIRECT_PRIORITY: u8 = 10;

pub type DeviceRef = Arc<Mutex<dyn Device>>;

///Implement the device bus, owning device registrations and address mappings
pub struct DeviceBus {
    // Linear list of devices, allowing O(1) access by ID
    devices: Vec<DeviceRef>,
    // Mapping physical address ranges to Device IDs
    // Key: Start Address -> (End Address, DeviceId, RemapOffset)
    map: BTreeMap<usize, BusRange>,
    next_range_id: usize,
}

impl DeviceBus {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            map: BTreeMap::new(),
            next_range_id: 0,
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
            kind: super::range::RangeKind::Device,
        };
        self.insert_range(range)
    }

    pub fn map_range(
        &mut self,
        start: usize,
        len: usize,
        redirect: usize,
        priority: u8,
    ) -> BusResult<()> {
        if len == 0 {
            return Err(BusError::RedirectInvalid {
                source: start,
                size: len,
                target: redirect,
                reason: "zero-length range",
            });
        }

        let source_end = start.checked_add(len).ok_or(BusError::RedirectInvalid {
            source: start,
            size: len,
            target: redirect,
            reason: "source range overflow",
        })?;

        let target_end = redirect.checked_add(len).ok_or(BusError::RedirectInvalid {
            source: start,
            size: len,
            target: redirect,
            reason: "target range overflow",
        })?;

        let target_range = self
            .range_for_address(redirect)
            .ok_or(BusError::RedirectInvalid {
                source: start,
                size: len,
                target: redirect,
                reason: "redirect target is unmapped",
            })?;

        if target_end > target_range.bus_end {
            return Err(BusError::RedirectInvalid {
                source: start,
                size: len,
                target: redirect,
                reason: "redirect spans multiple ranges",
            });
        }

        let device_offset = target_range.device_offset + (redirect - target_range.bus_start);
        let range = BusRange {
            bus_start: start,
            bus_end: source_end,
            device_offset,
            device_id: target_range.device_id,
            priority,
            kind: RangeKind::Redirect,
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
    use crate::soc::device::{AccessContext, DeviceError, DeviceResult, Endianness};
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

        fn read(&mut self, offset: usize, out: &mut [u8], _ctx: AccessContext) -> DeviceResult<()> {
            let end = offset + out.len();
            if end > self.backing.len() {
                return Err(DeviceError::OutOfRange {
                    offset,
                    len: out.len(),
                    capacity: self.backing.len(),
                });
            }
            out.copy_from_slice(&self.backing[offset..end]);
            Ok(())
        }

        fn write(&mut self, offset: usize, data: &[u8], _ctx: AccessContext) -> DeviceResult<()> {
            let end = offset + data.len();
            if end > self.backing.len() {
                return Err(DeviceError::OutOfRange {
                    offset,
                    len: data.len(),
                    capacity: self.backing.len(),
                });
            }
            self.backing[offset..end].copy_from_slice(data);
            Ok(())
        }
    }

    fn read_byte(bus: &DeviceBus, address: usize) -> u8 {
        let mut handle = bus
            .resolve(address)
            .unwrap_or_else(|_| panic!("address 0x{address:X} should resolve"));
        let mut byte = [0u8; 1];
        handle
            .read(&mut byte, AccessContext::CPU)
            .expect("read byte");
        byte[0]
    }

    fn write_bytes(bus: &mut DeviceBus, address: usize, data: &[u8]) {
        let mut handle = bus
            .resolve(address)
            .unwrap_or_else(|_| panic!("address 0x{address:X} should resolve"));
        handle.write(data, AccessContext::CPU).expect("write bytes");
    }

    #[test]
    fn register_device_and_resolve_returns_expected_mapping() {
        let mut bus = DeviceBus::new();
        let probe = ProbeDevice::new("probe", 0x2000);
        bus.map_device(probe, 0x4000, DEVICE_PRIORITY)
            .expect("register device");

        let mut handle = bus.resolve(0x5000).expect("resolve mapped address");
        let pattern = [0xAA, 0xBB, 0xCC, 0xDD];
        handle
            .write(&pattern, AccessContext::CPU)
            .expect("write via bus handle");

        let mut verifier = bus.resolve(0x5000).expect("resolve for verification");
        let mut buf = [0u8; 4];
        verifier
            .read(&mut buf, AccessContext::CPU)
            .expect("round-trip read");
        assert_eq!(
            buf, pattern,
            "bus read should see same bytes at alias offset"
        );
    }

    #[test]
    fn redirect_range_aliases_target_bytes() {
        let mut bus = DeviceBus::new();
        let probe = ProbeDevice::new("probe", 0x3000);
        bus.map_device(probe, 0x4000, DEVICE_PRIORITY)
            .expect("register device");

        write_bytes(&mut bus, 0x4100, &[0xFE, 0xED]);
        bus.map_range(0x2000, 0x20, 0x4100, REDIRECT_PRIORITY)
            .expect("map redirect");

        let mut buf = [0u8; 2];
        let mut alias = bus.resolve(0x2000).expect("resolve alias");
        alias
            .read(&mut buf, AccessContext::CPU)
            .expect("read alias");
        assert_eq!(buf, [0xFE, 0xED], "alias should mirror source bytes");

        let mut alias_writer = bus.resolve(0x2000).expect("resolve alias for write");
        alias_writer
            .write(&[0xAA, 0x55], AccessContext::CPU)
            .expect("write via redirect range");

        let mut verify = [0u8; 2];
        let mut source = bus.resolve(0x4100).expect("resolve source");
        source
            .read(&mut verify, AccessContext::CPU)
            .expect("read source");
        assert_eq!(verify, [0xAA, 0x55], "redirect writes should hit target");
    }

    #[test]
    fn lower_priority_blocked_until_higher_removed() {
        let mut bus = DeviceBus::new();
        let high = ProbeDevice::with_fill("hi", 0x100, 0xAA);
        bus.map_device(high, 0x8000, DEVICE_PRIORITY + 5)
            .expect("register high priority device");

        let low_attempt = ProbeDevice::with_fill("lo", 0x100, 0x33);
        let err = bus.map_device(low_attempt, 0x8000, DEVICE_PRIORITY);
        assert!(
            matches!(err, Err(BusError::Overlap { .. })),
            "lower priority should be rejected"
        );

        bus.unmap(0x8000).expect("remove higher priority range");

        let low = ProbeDevice::with_fill("lo", 0x100, 0x33);
        bus.map_device(low, 0x8000, DEVICE_PRIORITY)
            .expect("register low priority after removal");

        assert_eq!(
            read_byte(&bus, 0x8000),
            0x33,
            "after removal, low priority device should back the range"
        );
    }

    #[test]
    fn higher_priority_creates_hole_in_lower_range() {
        let mut bus = DeviceBus::new();
        let low = ProbeDevice::with_fill("low", 0x200, 0x11);
        bus.map_device(low, 0x2000, DEVICE_PRIORITY)
            .expect("register low priority range");

        assert_eq!(read_byte(&bus, 0x2050), 0x11, "baseline low range");

        let high = ProbeDevice::with_fill("high", 0x40, 0xEE);
        bus.map_device(high, 0x2060, DEVICE_PRIORITY + 10)
            .expect("register high priority slice");

        assert_eq!(
            read_byte(&bus, 0x205F),
            0x11,
            "address before hole still hits low device"
        );
        assert_eq!(
            read_byte(&bus, 0x2065),
            0xEE,
            "hole address resolves to high priority device"
        );
        assert_eq!(
            read_byte(&bus, 0x2090),
            0x11,
            "address after hole maps back to low device"
        );
    }
}
