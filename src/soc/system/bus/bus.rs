use std::{
    collections::HashMap,
    sync::{atomic::{AtomicU64, Ordering}, Arc, RwLock},
};

use super::{
    device::Device,
    error::{BusError, BusResult},
    range::{BusRange, RangeKind, ResolvedRange},
};

const DEVICE_PRIORITY: u8 = 0;
const REDIRECT_PRIORITY: u8 = 10;

pub struct DeviceBus {
    bucket_bits: u8,
    devices: RwLock<Vec<Arc<dyn Device>>>,
    name_index: RwLock<HashMap<String, usize>>,
    buckets: RwLock<HashMap<u64, Vec<BusRange>>>,
    range_index: RwLock<HashMap<u64, Vec<u64>>>,
    redirect_index: RwLock<HashMap<(u64, u64), u64>>,
    next_range_id: AtomicU64,
}

impl DeviceBus {
    pub fn new(bucket_bits: u8) -> Self {
        assert!(bucket_bits < 63, "bucket_bits must be < 63");
        Self {
            bucket_bits,
            devices: RwLock::new(Vec::new()),
            name_index: RwLock::new(HashMap::new()),
            buckets: RwLock::new(HashMap::new()),
            range_index: RwLock::new(HashMap::new()),
            redirect_index: RwLock::new(HashMap::new()),
            next_range_id: AtomicU64::new(1),
        }
    }

    fn bucket_index(&self, address: u64) -> u64 {
        address >> self.bucket_bits
    }

    fn insert_segment(&self, entry: &mut Vec<BusRange>, segment: BusRange) -> BusResult<()> {
        if let Some(conflict) = entry
            .iter()
            .find(|existing| existing.priority == segment.priority && existing.overlaps(&segment))
        {
            let devices = self.devices.read().unwrap();
            let details = devices
                .get(conflict.device_id)
                .map(|d| format!("conflicts with device '{}'", d.name()))
                .unwrap_or_else(|| "conflicts with unknown device".into());
            return Err(BusError::Overlap {
                address: segment.bus_start,
                details,
            });
        }

        let pos = entry.iter().position(|existing| {
            existing.priority < segment.priority
                || (existing.priority == segment.priority && existing.bus_start > segment.bus_start)
        });
        match pos {
            Some(idx) => entry.insert(idx, segment),
            None => entry.push(segment),
        }
        Ok(())
    }

    fn add_range(
        &self,
        bus_start: u64,
        bus_end: u64,
        device_id: usize,
        device_offset: u64,
        priority: u8,
        kind: RangeKind,
    ) -> BusResult<u64> {
        if bus_end <= bus_start {
            return Err(BusError::Overlap {
                address: bus_start,
                details: "range is empty".into(),
            });
        }

        let id = self.next_range_id.fetch_add(1, Ordering::Relaxed);
        let mut touched = Vec::new();
        let start_idx = self.bucket_index(bus_start);
        let end_idx = self.bucket_index(bus_end - 1);
        let mut buckets = self.buckets.write().unwrap();

        let segment = BusRange {
            id,
            bus_start,
            bus_end,
            device_offset,
            device_id,
            priority,
            kind,
        };

        for idx in start_idx..=end_idx {
            let entry = buckets.entry(idx).or_default();
            self.insert_segment(entry, segment.clone())?;
            touched.push(idx);
        }

        self.range_index.write().unwrap().insert(id, touched);
        Ok(id)
    }

    fn remove_range(&self, range_id: u64) -> BusResult<bool> {
        let bucket_indices = match self.range_index.write().unwrap().remove(&range_id) {
            Some(indices) => indices,
            None => return Ok(false),
        };

        let mut buckets = self.buckets.write().unwrap();
        for idx in bucket_indices {
            if let Some(vec) = buckets.get_mut(&idx) {
                vec.retain(|segment| segment.id != range_id);
                if vec.is_empty() {
                    buckets.remove(&idx);
                }
            }
        }
        Ok(true)
    }

    pub fn register_device(&self, device: Arc<dyn Device>, base_address: u64) -> BusResult<()> {
        let span = device.span();
        if span.start != 0 || span.end <= span.start {
            return Err(BusError::InvalidDeviceSpan {
                device: device.name().to_string(),
            });
        }
        let size = span.end - span.start;
        let end = base_address
            .checked_add(size)
            .ok_or(BusError::Overlap {
                address: base_address,
                details: "range exceeds address space".into(),
            })?;

        let name = device.name().to_string();
        {
            let names = self.name_index.read().unwrap();
            if names.contains_key(&name) {
                return Err(BusError::Overlap {
                    address: base_address,
                    details: format!("device '{name}' already registered"),
                });
            }
        }

        let mut devices = self.devices.write().unwrap();
        let mut names = self.name_index.write().unwrap();
        let device_id = devices.len();
        devices.push(device);
        names.insert(name, device_id);

        self.add_range(base_address, end, device_id, 0, DEVICE_PRIORITY, RangeKind::Device)?;
        Ok(())
    }

    pub fn redirect(&self, source_start: u64, size: u64, target_start: u64) -> BusResult<()> {
        if size == 0 {
            return Err(BusError::RedirectInvalid {
                source: source_start,
                size,
                target: target_start,
                reason: "size must be greater than zero",
            });
        }

        let resolved = self.resolve(target_start)?;
        let target_end = target_start
            .checked_add(size)
            .ok_or(BusError::RedirectInvalid {
                source: source_start,
                size,
                target: target_start,
                reason: "target address overflow",
            })?;

        if target_end > resolved.bus_end {
            return Err(BusError::RedirectInvalid {
                source: source_start,
                size,
                target: target_start,
                reason: "target range crosses device boundary",
            });
        }

        let source_end = source_start
            .checked_add(size)
            .ok_or(BusError::RedirectInvalid {
                source: source_start,
                size,
                target: target_start,
                reason: "source address overflow",
            })?;
        let device_offset = resolved.device_offset + (target_start - resolved.bus_start);
        let range_id = self.add_range(
            source_start,
            source_end,
            resolved.device_id,
            device_offset,
            REDIRECT_PRIORITY,
            RangeKind::Redirect,
        )?;
        self.redirect_index
            .write()
            .unwrap()
            .insert((source_start, size), range_id);
        Ok(())
    }

    pub fn remove_redirect(&self, source_start: u64, size: u64) -> BusResult<bool> {
        let range_id = match self.redirect_index.write().unwrap().remove(&(source_start, size)) {
            Some(id) => id,
            None => return Ok(false),
        };
        self.remove_range(range_id)
    }

    pub fn resolve(&self, address: u64) -> BusResult<ResolvedRange> {
        let bucket_idx = self.bucket_index(address);
        let segment = {
            let buckets = self.buckets.read().unwrap();
            buckets
                .get(&bucket_idx)
                .and_then(|segments| segments.iter().find(|segment| segment.contains(address)).cloned())
        };

        let segment = segment.ok_or(BusError::NotMapped { address })?;
        let devices = self.devices.read().unwrap();
        let device = devices
            .get(segment.device_id)
            .cloned()
            .ok_or(BusError::NotMapped { address })?;

        Ok(ResolvedRange {
            device,
            bus_start: segment.bus_start,
            bus_end: segment.bus_end,
            device_offset: segment.device_offset,
            priority: segment.priority,
            device_id: segment.device_id,
        })
    }

    pub fn bytes_to_end(&self, address: u64) -> BusResult<u64> {
        let resolved = self.resolve(address)?;
        Ok(resolved.bus_end - address)
    }
}
