pub mod patch;
pub mod pointer;
pub mod serializer;
pub mod string_block;

use crate::common::*;
use crate::ser::patch::Patch;

// TODO: set reverse map
const RSVMAP_LEN: usize = 16;

/// Serialize the data to dtb, with a list fof Patch, write to the `writer`.
///
/// We do run-twice on convert, first time to generate string block, second time todo real
/// structure.
pub fn to_dtb<'se, T>(data: &T, list: &'se [Patch<'se>], writer: &'se mut [u8]) -> Result<(), Error>
where
    T: serde::ser::Serialize,
{
    writer.iter_mut().for_each(|x| *x = 0);

    let string_block_length = {
        let mut offset: usize = 0;
        {
            let mut dst = crate::ser::pointer::Pointer::new(None);
            let mut patch_list = crate::ser::patch::PatchList::new(list);
            let mut block = crate::ser::string_block::StringBlock::new(writer, &mut offset);
            let mut ser =
                crate::ser::serializer::SerializerInner::new(&mut dst, &mut block, &mut patch_list);
            let ser = crate::ser::serializer::Serializer::new(&mut ser);
            data.serialize(ser)?;
            offset
        };
        {
            let mut block = crate::ser::string_block::StringBlock::new(writer, &mut offset);
            block.align();
        };
        offset
    };
    list.iter().for_each(|patch| patch.init());
    // Write from bottom to top, to avoid overlap.
    for i in (0..string_block_length).rev() {
        writer[writer.len() - string_block_length + i] = writer[i];
        writer[i] = 0;
    }

    let struct_len = {
        let (data_block, string_block) = writer.split_at_mut(writer.len() - string_block_length);
        let (_, data_block) = data_block.split_at_mut(HEADER_PADDING_LEN as usize + RSVMAP_LEN);
        let mut patch_list = crate::ser::patch::PatchList::new(list);
        let mut temp_length = string_block_length;
        let mut block = crate::ser::string_block::StringBlock::new(string_block, &mut temp_length);
        let mut dst = crate::ser::pointer::Pointer::new(Some(data_block));
        let mut ser =
            crate::ser::serializer::SerializerInner::new(&mut dst, &mut block, &mut patch_list);
        let ser = crate::ser::serializer::Serializer::new(&mut ser);
        let struct_len = data.serialize(ser)?.1;
        assert_eq!(struct_len % 4, 0); // As spec, structure block align with 4 bytes.
        assert_eq!(temp_length, string_block_length); // StringBlock should be same with first run.
        struct_len
    };

    // Align to 8-bytes.
    for i in 0..string_block_length {
        writer[HEADER_PADDING_LEN as usize + RSVMAP_LEN + struct_len + i] =
            writer[writer.len() - string_block_length + i];
        writer[writer.len() - string_block_length + i] = 0;
    }

    // Make header
    {
        let (header, _) = writer.split_at_mut(HEADER_LEN as usize);
        let header = unsafe { &mut *(header.as_mut_ptr() as *mut Header) };
        header.magic = u32::from_be(DEVICE_TREE_MAGIC);
        header.total_size = u32::from_be(
            HEADER_PADDING_LEN + (RSVMAP_LEN + struct_len + string_block_length) as u32,
        );
        assert_eq!(header.total_size % 8, 0);
        header.off_dt_struct = u32::from_be(HEADER_PADDING_LEN + RSVMAP_LEN as u32);
        header.off_dt_strings = u32::from_be(HEADER_PADDING_LEN + (RSVMAP_LEN + struct_len) as u32);
        header.off_mem_rsvmap = u32::from_be(HEADER_PADDING_LEN);
        header.version = u32::from_be(SUPPORTED_VERSION);
        header.last_comp_version = u32::from_be(SUPPORTED_VERSION); // TODO: maybe 16
        header.boot_cpuid_phys = 0; // TODO
        header.size_dt_strings = u32::from_be(string_block_length as u32);
        header.size_dt_struct = u32::from_be(struct_len as u32);
    }
    Ok(())
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
