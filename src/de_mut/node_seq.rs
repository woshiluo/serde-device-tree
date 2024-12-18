﻿use super::{BodyCursor, GroupCursor, RefDtb, RegConfig, StructDeserializer};
use crate::de_mut::GroupDeserializer;
use core::{fmt::Debug, marker::PhantomData};
use serde::{de, Deserialize};

/// 一组名字以 `@...` 区分，同类、同级且连续的节点的映射。
///
/// 在解析前，无法得知这种节点的数量，因此也无法为它们分配足够的空间，
/// 因此这些节点将延迟解析。
/// 迭代 `NodeSeq` 可获得一系列 [`NodeSeqItem`]，再调用 `deserialize` 方法分别解析每个节点。
pub struct NodeSeq<'de> {
    inner: GroupDeserializer<'de>,
}

/// 连续节点迭代器。
pub struct NodeSeqIter<'de, 'b> {
    seq: &'b NodeSeq<'de>,
    cursor: GroupCursor,
    i: usize,
}

/// 连续节点对象。
pub struct NodeSeqItem<'de> {
    dtb: RefDtb<'de>,
    reg: RegConfig,
    body: BodyCursor,
    at: &'de str,
}

impl<'de> Deserialize<'de> for NodeSeq<'_> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<'de, 'b> {
            marker: PhantomData<NodeSeq<'b>>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de, 'b> de::Visitor<'de> for Visitor<'de, 'b> {
            type Value = NodeSeq<'b>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(formatter, "struct StrSeq")
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                // 结构体转为内存切片，然后拷贝过来
                if v.len() == core::mem::size_of::<Self::Value>() {
                    Ok(Self::Value::from_raw_inner_parts(v.as_ptr()))
                } else {
                    Err(E::invalid_length(
                        v.len(),
                        &"`NodeSeq` is copied with wrong size.",
                    ))
                }
            }
        }

        serde::Deserializer::deserialize_newtype_struct(
            deserializer,
            "NodeSeq",
            Visitor {
                marker: PhantomData,
                lifetime: PhantomData,
            },
        )
    }
}

impl<'de> NodeSeq<'de> {
    fn from_raw_inner_parts(ptr: *const u8) -> Self {
        // 直接从指针拷贝
        let original_inner = unsafe { &*(ptr as *const GroupDeserializer<'_>) };
        let res = Self {
            inner: *original_inner,
        };
        // 初始化
        res.inner
            .cursor
            .init_on(res.inner.dtb, res.inner.len_item, res.inner.len_name);
        res
    }

    /// 连续节点总数。
    pub const fn len(&self) -> usize {
        self.inner.len_item
    }

    /// 如果连续节点数量为零，返回 true。但连续节点数量不可能为零。
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// 获得节点迭代器。
    pub const fn iter<'b>(&'b self) -> NodeSeqIter<'de, 'b> {
        NodeSeqIter {
            seq: self,
            cursor: self.inner.cursor,
            i: 0,
        }
    }
}

impl Debug for NodeSeq<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut iter = self.iter();
        if let Some(s) = iter.next() {
            write!(f, "[@{}", s.at)?;
            for s in iter {
                write!(f, ", @{}", s.at)?;
            }
            write!(f, "]")
        } else {
            unreachable!("NodeSeq contains at least one node.")
        }
    }
}

impl Drop for NodeSeq<'_> {
    fn drop(&mut self) {
        self.inner
            .cursor
            .drop_on(self.inner.dtb, self.inner.len_item);
    }
}

impl<'de> Iterator for NodeSeqIter<'de, '_> {
    type Item = NodeSeqItem<'de>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.seq.inner.len_item {
            None
        } else {
            self.i += 1;
            let dtb = self.seq.inner.dtb;
            let (name, body) = self.cursor.name_on(dtb);
            let off_next = self.cursor.offset_on(dtb);
            self.cursor.step_n(off_next);
            Some(Self::Item {
                dtb,
                reg: self.seq.inner.reg,
                body,
                at: unsafe { core::str::from_utf8_unchecked(&name[self.seq.inner.len_name + 1..]) },
            })
        }
    }
}

impl NodeSeqItem<'_> {
    /// 获得区分节点的序号。
    pub fn at(&self) -> &str {
        self.at
    }
}

impl<'de> NodeSeqItem<'de> {
    /// 反序列化一个节点的内容。
    pub fn deserialize<T: Deserialize<'de>>(&self) -> T {
        T::deserialize(&mut StructDeserializer {
            dtb: self.dtb,
            reg: self.reg,
            cursor: self.body,
        })
        .unwrap()
    }
}
