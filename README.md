# KVDB

A simple key value pair way of storing data!

## How it works

- It will store bytes in a file and it will keep an internal pointer to the position of the entry.
- It has a BufReader and BufWriter reading and inserting entries in the file when you insert and retrieve a key.

## How to use

- Simply create a struct that implements From<Vec<u8>> and Into<Vec<u8>> and you are all done :).

## Examples

```rs
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
```
