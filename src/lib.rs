use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, Error, ErrorKind, Result, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Kvdb {
    local_mem: HashMap<Vec<u8>, usize>,
    current_pos: usize,
    reader: Option<BufReader<fs::File>>,
    writer: Option<BufWriter<fs::File>>,
    f: Option<fs::File>,
}

impl Kvdb {
    pub fn new() -> Self {
        Self {
            local_mem: HashMap::default(),
            reader: None,
            writer: None,
            current_pos: 0,
            f: None,
        }
    }

    ///
    /// will create/load a key value pair data storage
    ///
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let handler = OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(path)?;
        let write_handler = handler.try_clone()?;
        let read_handler = handler.try_clone()?;
        self.f = Some(handler);
        self.reader = Some(BufReader::new(read_handler));
        self.writer = Some(BufWriter::new(write_handler));
        self.load_into_hashmap()?;
        Ok(())
    }

    fn load_into_hashmap(&mut self) -> Result<()> {
        let reader = self.reader.as_mut().expect("reader is empty");
        let mut position = 0;
        loop {
            let mut key_size_buff: [u8; 8] = [0; 8];
            match reader.read_exact(&mut key_size_buff) {
                Ok(()) => {
                    let key_size = usize::from_le_bytes(key_size_buff);
                    reader.read_exact(&mut key_size_buff)?;
                    let value_size = usize::from_le_bytes(key_size_buff);
                    let mut vec_key = vec![0u8; key_size];
                    reader.read(&mut vec_key)?;
                    reader.seek(SeekFrom::Current((value_size) as i64))?;
                    self.local_mem.insert(vec_key, position);
                    position += std::mem::size_of::<usize>() * 2 + value_size + key_size;
                }
                Err(err) => match err.kind() {
                    ErrorKind::UnexpectedEof => {
                        reader.seek(SeekFrom::Start(0))?;
                        self.current_pos = position;
                        break;
                    }
                    _ => {
                        return Err(err);
                    }
                },
            }
        }
        Ok(())
    }

    fn get_by_key_ref<V: From<Vec<u8>>>(&mut self, key: &Vec<u8>) -> Result<V> {
        let reader = self.reader.as_mut().unwrap();
        let pos = self.local_mem.get(key);
        if let Some(pos) = pos {
            // put pointer into read position
            reader.seek(SeekFrom::Start(*pos as u64))?;
            let mut size_buff: [u8; 8] = [0; 8];
            // retrieve key size for the future
            reader.read_exact(&mut size_buff)?;
            let key_size = usize::from_le_bytes(size_buff);
            // retrieve val size
            reader.read_exact(&mut size_buff)?;
            let val_size = usize::from_le_bytes(size_buff);
            if val_size != 0 {
                let mut vec_key = vec![0u8; key_size];
                reader.read(&mut vec_key)?;
                // Read val
                let mut vec = vec![0u8; val_size];
                reader.read(&mut vec)?;
                return Ok(V::from(vec));
            }
        }
        Result::Err(Error::from(ErrorKind::NotFound))
    }

    ///
    /// retrieves a key value pair, will return an error if it doesn't exist
    ///
    pub fn get<T: Into<Vec<u8>>, V: From<Vec<u8>>>(&mut self, key: T) -> Result<V> {
        let k_buff = key.into();
        return self.get_by_key_ref(&k_buff);
    }

    ///
    /// deletes a key value pair
    ///
    /// it won't delete it in the file, but will insert
    /// the key again but with the 0 value, so the
    /// system knows that it is a deleted pair
    ///
    pub fn delete<T: Into<Vec<u8>>, V: Into<Vec<u8>> + From<Vec<u8>>>(
        &mut self,
        key: T,
    ) -> Result<V> {
        let buff = key.into();
        // We check that it indeed exists
        let result = self.get_by_key_ref::<V>(&buff);
        if let Err(err) = result {
            return Err(err);
        }
        // unwrap the result safely
        let v = result.unwrap();
        // empty vector to insert into the file
        let empty = Vec::with_capacity(0);
        // override previous value
        self.insert_by_key_ref(&buff, &empty)?;
        Ok(v)
    }

    ///
    /// insert method used by `delete` and `insert`
    /// - it is used by delete because if you pass an empty vector to the value, it will basically
    ///   delete the key
    /// useful because it borrows the key and value
    ///
    fn insert_by_key_ref(&mut self, key: &Vec<u8>, val: &Vec<u8>) -> Result<()> {
        let writer = self.writer.as_mut().unwrap();
        // just in case, go to the end of the file
        writer.seek(SeekFrom::End(0))?;
        let len = key.len();
        let initial_pos = self.current_pos;
        self.current_pos += writer.write(&usize::to_le_bytes(len))?;
        self.current_pos += writer.write(&usize::to_le_bytes(val.len()))?;
        self.current_pos += writer.write(key)?;
        self.current_pos += writer.write(val)?;
        self.local_mem.insert(key.clone(), initial_pos);
        writer.flush()?;
        Ok(())
    }

    ///
    /// inserts a key value pair into the file
    ///
    pub fn insert<T: Into<Vec<u8>>, V: Into<Vec<u8>>>(&mut self, key: T, val: V) -> Result<()> {
        let key = key.into();
        let val = val.into();
        self.insert_by_key_ref(&key, &val)
    }
}

#[cfg(test)]
mod tests {
    #[derive(Debug)]
    struct TestValue {
        value: String,
    }

    impl From<Vec<u8>> for TestValue {
        fn from(buff: Vec<u8>) -> Self {
            Self {
                value: std::str::from_utf8(&buff).unwrap().to_string(),
            }
        }
    }

    impl Into<Vec<u8>> for TestValue {
        fn into(self) -> Vec<u8> {
            self.value.into_bytes()
        }
    }

    use super::Kvdb;
    fn pure_inserting_works() {
        let mut kv = Kvdb::new();
        kv.load("./data").expect("expect load to work");
        for i in 0..100 {
            let string = String::from(format!("test{}", i));
            let value: TestValue = TestValue {
                value: string.clone(),
            };
            kv.insert(format!("key_test{}", i), value)
                .expect("expect this to work!");
            let val: TestValue = kv.get(format!("key_test{}", i)).unwrap();
            assert_eq!(string, val.value);
        }
    }
    #[test]
    fn load_works() {
        pure_inserting_works();
        let mut kv = Kvdb::new();
        if let Err(err) = kv.load("./data") {
            println!("error: {:?}", err);
            return;
        }
        assert_eq!(
            kv.delete::<_, TestValue>("key_test0").unwrap().value,
            "test0"
        );
        let shouldnt_exist = kv.get::<_, TestValue>("key_test0");
        matches!(shouldnt_exist, Err(_err));
        kv.insert(
            "key_test0",
            TestValue {
                value: "test0".to_string(),
            },
        )
        .expect("expect the insert to work");
        for i in 0..100 {
            let val: TestValue = kv.get(format!("key_test{}", i)).unwrap();
            assert_eq!(val.value, format!("test{}", i));
        }
    }
}
