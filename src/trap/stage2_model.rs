use std::collections::BTreeMap;

use super::stage2::{MemoryType, PageSize, Stage2Mapping, Stage2Permissions};
use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct MappingKey {
    owner_vm: String,
    address_space: String,
    base: u64,
}

impl MappingKey {
    fn new(mapping: &Stage2Mapping) -> Self {
        Self {
            owner_vm: mapping.owner_vm.clone(),
            address_space: mapping.address_space.clone(),
            base: mapping.base,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Stage2Table {
    mappings: BTreeMap<MappingKey, Stage2Mapping>,
}

impl Stage2Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub fn map(&mut self, mapping: Stage2Mapping) -> Result<(), TrapError> {
        self.reject_overlap(&mapping)?;
        self.mappings.insert(MappingKey::new(&mapping), mapping);
        Ok(())
    }

    pub fn lookup(&self, owner_vm: &str, address_space: &str, gpa: u64) -> Option<&Stage2Mapping> {
        self.mappings.values().find(|mapping| {
            mapping.owner_vm == owner_vm
                && mapping.address_space == address_space
                && mapping.contains(gpa)
        })
    }

    pub fn lookup_mut(
        &mut self,
        owner_vm: &str,
        address_space: &str,
        gpa: u64,
    ) -> Option<&mut Stage2Mapping> {
        self.mappings.values_mut().find(|mapping| {
            mapping.owner_vm == owner_vm
                && mapping.address_space == address_space
                && mapping.contains(gpa)
        })
    }

    pub fn set_permissions(
        &mut self,
        owner_vm: &str,
        address_space: &str,
        gpa: u64,
        permissions: Stage2Permissions,
    ) -> Result<Stage2Permissions, TrapError> {
        let mapping = self
            .lookup_mut(owner_vm, address_space, gpa)
            .ok_or_else(|| missing_mapping(owner_vm, address_space, gpa))?;
        let previous = mapping.permissions;
        mapping.permissions = permissions;
        Ok(previous)
    }

    pub fn split(
        &mut self,
        owner_vm: &str,
        address_space: &str,
        base: u64,
        target: PageSize,
    ) -> Result<Vec<Stage2Mapping>, TrapError> {
        let key = MappingKey {
            owner_vm: owner_vm.to_string(),
            address_space: address_space.to_string(),
            base,
        };
        let parent = self
            .mappings
            .remove(&key)
            .ok_or_else(|| missing_mapping(owner_vm, address_space, base))?;
        let count = parent.page_size.split_count(target)?;
        let mut children = Vec::with_capacity(count as usize);
        for index in 0..count {
            let child_base = base + index * target.bytes();
            let child = Stage2Mapping::new(
                parent.owner_vm.clone(),
                parent.address_space.clone(),
                child_base,
                target,
                parent.memory_type,
                parent.permissions,
            )?;
            self.mappings.insert(MappingKey::new(&child), child.clone());
            children.push(child);
        }
        Ok(children)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Stage2Mapping> {
        self.mappings.values()
    }

    fn reject_overlap(&self, candidate: &Stage2Mapping) -> Result<(), TrapError> {
        let candidate_end = candidate.end()?;
        for existing in self.mappings.values() {
            if existing.owner_vm != candidate.owner_vm
                || existing.address_space != candidate.address_space
            {
                continue;
            }
            let existing_end = existing.end()?;
            if candidate.base < existing_end && existing.base < candidate_end {
                return Err(TrapError::new(
                    TrapErrorKind::Overlap,
                    format!(
                        "stage-2 mapping {:#x}..{:#x} overlaps existing mapping {:#x}..{:#x} for vm={} address_space={}",
                        candidate.base,
                        candidate_end,
                        existing.base,
                        existing_end,
                        candidate.owner_vm,
                        candidate.address_space
                    ),
                ));
            }
        }
        Ok(())
    }
}

pub fn synthetic_mapping(
    owner_vm: &str,
    address_space: &str,
    base: u64,
    page_size: PageSize,
    permissions: Stage2Permissions,
) -> Result<Stage2Mapping, TrapError> {
    Stage2Mapping::new(
        owner_vm,
        address_space,
        base,
        page_size,
        MemoryType::WriteBack,
        permissions,
    )
}

fn missing_mapping(owner_vm: &str, address_space: &str, gpa: u64) -> TrapError {
    TrapError::new(
        TrapErrorKind::NotMapped,
        format!(
            "no stage-2 mapping covers {gpa:#x} for vm={owner_vm} address_space={address_space}"
        ),
    )
}
