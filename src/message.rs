use std::any::TypeId;
use std::{mem, ptr};
use serde::{ser::Serialize, de::DeserializeOwned};
use std::fmt::{self, Debug};
use bincode;

// can only be passed in the same thread
pub trait Message: Debug {}
impl<T: Debug> Message for T {}

type Error = bincode::Error;

// can be encoded
pub trait Sendable: Sized + Serialize + DeserializeOwned + 'static {
    fn encode(&self, mut buffer: &mut Vec<u8>) -> Result<(), Error> {
        let type_id = unsafe {
            mem::transmute::<TypeId, u64>(TypeId::of::<Self>())
        }; // well, what can I do …
        let size = bincode::serialized_size(self)?;
        bincode::serialize_into(&mut buffer, &size)?;
        bincode::serialize_into(&mut buffer, self)?;
        bincode::serialize_into(&mut buffer, &type_id)
    }
    
    fn decode(mut data: &mut &[u8]) -> Result<Self, Error> {
        let type_id = bincode::deserialize_from(&mut data)?;
        let type_id = unsafe {
            mem::transmute::<u64, TypeId>(type_id)
        }; // … not much.
        assert_eq!(type_id, TypeId::of::<Self>());
        
        let size: u32 = bincode::deserialize_from(&mut data)?;
        let remaining_data_len = data.len() - size as usize;
        let event: Self = bincode::deserialize_from(&mut data)?;
        assert_eq!(data.len(), remaining_data_len);
        
        Ok(event)
    }
}

type Payload = [usize; 2];
#[derive(Debug)]
enum Data {
    Inline(Payload),
    Box(Box<dyn Message>)
}

pub struct Envelope {
    event: Data,
    pub type_id: TypeId
}

const fn is_inline<T>() -> bool {
    !mem::needs_drop::<T>() &&
    mem::size_of::<T>() <= mem::size_of::<Payload>() &&
    mem::align_of::<T>() <= mem::align_of::<Payload>()
}

/* envelope is encoded as:
  type_id u64
  len     u32
  data    len bytes
*/

impl Envelope {
    pub fn pack<T: Message + 'static>(e: T) -> Envelope {
        let event = if is_inline::<T>() {
            let mut data = [0; 2];
            unsafe {
                ptr::write(data.as_mut_ptr() as *mut T, e);
            }
            Data::Inline(data)
        } else {
            Data::Box(Box::new(e))
        };

        Envelope {
            event,
            type_id: TypeId::of::<T>()
        }
    }
    pub fn unpack<T: Message + 'static>(self) -> T {
        let Envelope { event, type_id } = self;
        assert_eq!(type_id, TypeId::of::<T>());
        
        if is_inline::<T>() {
            match event {
                Data::Inline(data) => unsafe {
                    ptr::read::<T>(&data as *const usize as *const T)
                }
                Data::Box(_) => unreachable!()
            }
        } else {
            match event {
                Data::Box(b) => unsafe {
                    let ptr = Box::into_raw(b);
                    *Box::from_raw(ptr as *mut T)
                }
                Data::Inline(_) => unreachable!()
            }
        }
    }
}
impl Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.event.fmt(f)
    }
}
