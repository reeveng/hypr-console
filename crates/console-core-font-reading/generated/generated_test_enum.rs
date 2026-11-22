

use crate::codegen_prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "the generated fixture enum retains the byte codes exercised by its encoded test records")
)]
pub enum MyEnum1 {
    #[default]
    ItsAZero = 0,
    ItsAOne = 1,
    #[doc(hidden)]
    Unknown,
}

impl MyEnum1 {
    fn discriminant_value(self) -> Result<u16, Never> {
        Ok(match self {
            Self::ItsAZero => 0,
            Self::ItsAOne => 1,
            Self::Unknown => 2,
        })
    }

    pub fn new(raw: u16) -> Result<Self, Never> {
        Ok(match raw {
            0 => Self::ItsAZero,
            1 => Self::ItsAOne,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for MyEnum1 {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_2) = <u16>::from_raw(raw);

        let t = value_2;
        let value_3 = t;

        let Ok(my_enum1) = Self::new(value_3);

        Ok(my_enum1)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<MyEnum1> for FieldType<'a> {
    fn from(src: MyEnum1) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "the generated fixture enum retains the byte codes exercised by its encoded test records")
)]
pub enum MyEnum2 {
    ItsATwo = 2,
    #[default]
    ItsAThree = 3,
    #[doc(hidden)]
    Unknown,
}

impl MyEnum2 {
    fn discriminant_value(self) -> Result<u16, Never> {
        Ok(match self {
            Self::ItsATwo => 2,
            Self::ItsAThree => 3,
            Self::Unknown => 4,
        })
    }

    pub fn new(raw: u16) -> Result<Self, Never> {
        Ok(match raw {
            2 => Self::ItsATwo,
            3 => Self::ItsAThree,
            _ => Self::Unknown,
        })
    }
}

impl font_types::Scalar for MyEnum2 {
    type Raw = <u16 as font_types::Scalar>::Raw;

    fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
        let Ok(value) = self.discriminant_value();
        value.to_raw()
    }

    fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
        let Ok(value_5) = <u16>::from_raw(raw);

        let t = value_5;
        let value_6 = t;

        let Ok(my_enum2) = Self::new(value_6);

        Ok(my_enum2)
    }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> From<MyEnum2> for FieldType<'a> {
    fn from(src: MyEnum2) -> FieldType<'a> {
        let Ok(value) = src.discriminant_value();
        FieldType::from(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck :: AnyBitPattern)]
#[repr(C)]
pub struct MyRecord {
    pub my_enum1: BigEndian<MyEnum1>,
    pub my_enum2: BigEndian<MyEnum2>,
}

const _: () = {
  assert!(core::mem::size_of::<MyRecord>() == 4);
  assert!(core::mem::align_of::<MyRecord>() == 1);
  assert!(core::mem::offset_of!(MyRecord, my_enum1) == 0);
  assert!(core::mem::offset_of!(MyRecord, my_enum2) == 2);
};

impl MyRecord {
    pub fn my_enum1(&self) -> Result<MyEnum1, Never> {
        self.my_enum1.get()
    }

    pub fn my_enum2(&self) -> Result<MyEnum2, Never> {
        self.my_enum2.get()
    }
}

impl FixedSize for MyRecord {
    const RAW_BYTE_LEN: u64 = MyEnum1::RAW_BYTE_LEN + MyEnum2::RAW_BYTE_LEN;
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for MyRecord {
    fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
        Ok(RecordResolver {
            name: "MyRecord",
            get_field: Box::new(move |idx, _data| match idx {
                0u64 => Some(Field { name: "my_enum1", value: FieldType::from(self.my_enum1()) }),
                1u64 => Some(Field { name: "my_enum2", value: FieldType::from(self.my_enum2()) }),
                _ => None,
            }),
            data,
        })
    }
}
