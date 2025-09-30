pub mod patch;
pub mod pointer;
pub mod serializer;

pub mod string_block;

use crate::common::*;
use crate::ser::patch::Patch;

// TODO: set reverse map
const RSVMAP_LEN: usize = 16;

#[inline(always)]
fn get_structure_padding(length: usize) -> usize {
    let rem = length & (ALIGN - 1);
    ALIGN - rem
}

/// Make dtb header with structure block and string block length.
fn make_header<'se>(
    writer: &'se mut [u8],
    structure_length: u32,
    string_block_length: u32,
) -> usize {
    let (header, _) = writer.split_at_mut(HEADER_LEN as usize);
    let header = unsafe { &mut *(header.as_mut_ptr() as *mut Header) };
    let total_size =
        HEADER_PADDING_LEN + RSVMAP_LEN as u32 + structure_length + string_block_length;
    let padding = get_structure_padding(total_size as usize);
    let total_size = total_size + padding as u32;
    header.magic = u32::from_be(DEVICE_TREE_MAGIC);
    header.total_size = u32::from_be(total_size);
    assert_eq!(header.total_size % 8, 0);
    header.off_dt_struct = u32::from_be(HEADER_PADDING_LEN + RSVMAP_LEN as u32);
    header.off_dt_strings = u32::from_be(HEADER_PADDING_LEN + RSVMAP_LEN as u32 + structure_length);
    header.off_mem_rsvmap = u32::from_be(HEADER_PADDING_LEN);
    header.version = u32::from_be(SUPPORTED_VERSION);
    header.last_comp_version = u32::from_be(SUPPORTED_VERSION); // TODO: maybe 16
    header.boot_cpuid_phys = 0; // TODO
    header.size_dt_strings = u32::from_be(string_block_length as u32);
    header.size_dt_struct = u32::from_be(structure_length as u32);

    total_size as usize
}

/// Serialize the data to dtb, with a list fof Patch, write to the `writer`.
///
/// We do run-twice on convert, first time to generate string block, second time todo real
/// structure.
pub fn to_dtb<'se, T>(
    data: &T,
    list: &'se [Patch<'se>],
    writer: &'se mut [u8],
) -> Result<usize, Error>
where
    T: serde::ser::Serialize,
{
    writer.fill(0);

    let (string_block_length, structure_length) = {
        let mut offset: usize = 0;
        let mut block = crate::ser::string_block::StringBlock::new(Some(writer), &mut offset);
        let mut dst = crate::ser::pointer::Pointer::new(None);
        let mut patch_list = crate::ser::patch::PatchList::new(list);
        let mut ser =
            crate::ser::serializer::SerializerInner::new(&mut dst, &mut block, &mut patch_list);
        let ser = crate::ser::serializer::Serializer::new(&mut ser);
        let structure_length = data.serialize(ser)?.1;
        (offset, structure_length)
    };

    // Clear string block
    writer[0..string_block_length].fill(0);
    list.iter().for_each(|patch| patch.init());
    let string_block_start = HEADER_PADDING_LEN as usize + RSVMAP_LEN + structure_length;

    {
        let (data_block, string_block) = writer.split_at_mut(string_block_start);
        let (_, data_block) = data_block.split_at_mut(HEADER_PADDING_LEN as usize + RSVMAP_LEN);

        let mut patch_list = crate::ser::patch::PatchList::new(list);
        let mut temp_length = 0;
        let mut block =
            crate::ser::string_block::StringBlock::new(Some(string_block), &mut temp_length);
        let mut dst = crate::ser::pointer::Pointer::new(Some(data_block));
        let mut ser =
            crate::ser::serializer::SerializerInner::new(&mut dst, &mut block, &mut patch_list);
        let ser = crate::ser::serializer::Serializer::new(&mut ser);
        let struct_len = data.serialize(ser)?.1;
        assert_eq!(temp_length, string_block_length); // StringBlock should be same with first run.
        struct_len
    };

    let result = make_header(writer, structure_length as u32, string_block_length as u32);

    Ok(result)
}

#[cfg(feature = "alloc")]
pub fn probe_dtb_length<'se, T>(data: &T, list: &'se [Patch<'se>]) -> Result<usize, Error>
where
    T: serde::ser::Serialize,
{
    let mut offset: usize = 0;
    let structure_length = {
        let mut dst = crate::ser::pointer::Pointer::new(None);
        let mut patch_list = crate::ser::patch::PatchList::new(list);
        let mut block = crate::ser::string_block::StringBlock::new(None, &mut offset);
        let mut ser =
            crate::ser::serializer::SerializerInner::new(&mut dst, &mut block, &mut patch_list);
        let ser = crate::ser::serializer::Serializer::new(&mut ser);
        data.serialize(ser)?.1
    };

    let total_size = HEADER_PADDING_LEN as usize + RSVMAP_LEN + structure_length + offset;
    let padding = get_structure_padding(total_size);
    let total_size = total_size + padding;
    Ok(total_size)
}

#[derive(Debug)]
pub enum Error {
    Unknown,
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}", self)
    }
}

impl core::error::Error for Error {}

impl serde::ser::Error for Error {
    fn custom<T>(_msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self::Unknown
    }
}
