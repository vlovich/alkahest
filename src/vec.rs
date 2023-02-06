use alloc::vec::Vec;

use crate::{
    bytes::Bytes,
    deserialize::{Deserialize, Deserializer, Error},
    formula::Formula,
    reference::Ref,
    serialize::{SerializeOwned, Serializer},
    Serialize,
};

impl<F> Formula for Vec<F>
where
    F: Formula,
{
    const MAX_SIZE: Option<usize> = <Ref<[F]> as Formula>::MAX_SIZE;

    type NonRef = [F];

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn serialize<T, S>(value: T, ser: impl Into<S>) -> Result<S::Ok, S::Error>
    where
        T: SerializeOwned<[F]>,
        S: Serializer,
    {
        <Ref<[F]>>::serialize(value, ser)
    }

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize<'de, T>(de: Deserializer<'de>) -> Result<T, Error>
    where
        T: Deserialize<'de, [F]>,
    {
        <Ref<[F]>>::deserialize(de)
    }

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize_in_place<'de, T>(place: &mut T, de: Deserializer<'de>) -> Result<(), Error>
    where
        T: Deserialize<'de, [F]> + ?Sized,
    {
        <Ref<[F]>>::deserialize_in_place(place, de)
    }
}

impl<'de, F, T, const N: usize> Deserialize<'de, [F; N]> for Vec<T>
where
    F: Formula,
    T: Deserialize<'de, F::NonRef>,
{
    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize(de: Deserializer<'de>) -> Result<Self, Error> {
        de.into_iter::<F, T>()?.collect()
    }

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize_in_place(&mut self, de: Deserializer<'de>) -> Result<(), Error> {
        let iter = de.into_iter::<F, T>()?;
        self.reserve(iter.len());
        for elem in iter {
            self.push(elem?);
        }
        Ok(())
    }
}

impl<F, T> SerializeOwned<[F]> for Vec<T>
where
    T: SerializeOwned<F::NonRef>,
    F: Formula,
{
    fn serialize_owned<S>(self, er: impl Into<S>) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut er = er.into();
        for elem in self {
            er.write_value::<F, _>(elem)?;
        }
        er.finish()
    }
}

impl<F, T> Serialize<[F]> for Vec<T>
where
    T: Serialize<F::NonRef>,
    F: Formula,
{
    fn serialize<S>(&self, er: impl Into<S>) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut er = er.into();
        for elem in self {
            er.write_value::<F, _>(elem)?;
        }
        er.finish()
    }
}

impl<'de, F, T> Deserialize<'de, [F]> for Vec<T>
where
    F: Formula,
    T: Deserialize<'de, F::NonRef>,
{
    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize(de: Deserializer<'de>) -> Result<Self, Error> {
        de.into_iter::<F, T>()?.collect()
    }

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize_in_place(&mut self, de: Deserializer<'de>) -> Result<(), Error> {
        let iter = de.into_iter::<F, T>()?;
        self.reserve(iter.len());
        for elem in iter {
            self.push(elem?);
        }
        Ok(())
    }
}

impl SerializeOwned<Bytes> for Vec<u8> {
    #[cfg_attr(feature = "inline-more", inline(always))]
    fn serialize_owned<S>(self, ser: impl Into<S>) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Serialize::<Bytes>::serialize(&self, ser)
    }
}

impl Serialize<Bytes> for Vec<u8> {
    #[cfg_attr(feature = "inline-more", inline(always))]
    fn serialize<S>(&self, ser: impl Into<S>) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.into();
        ser.write_bytes(&self)?;
        ser.finish()
    }
}

impl<'de> Deserialize<'de, Bytes> for Vec<u8> {
    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize(de: Deserializer) -> Result<Self, Error> {
        Ok(de.read_all_bytes().to_vec())
    }

    #[cfg_attr(feature = "inline-more", inline(always))]
    fn deserialize_in_place(&mut self, de: Deserializer) -> Result<(), Error> {
        self.extend_from_slice(de.read_all_bytes());
        Ok(())
    }
}
