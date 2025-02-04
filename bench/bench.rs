use binggan::{black_box, plugins::*, InputGroup, PeakMemAlloc, INSTRUMENTED_SYSTEM};
use serde_json::json;
use std::collections::BTreeMap;
pub use vrl::value::{Secrets as VrlSecrets, Value as VrlValue};
use vrl::{compiler::runtime::Runtime, prelude::state::RuntimeState};
use vrl::{
    compiler::{Program, TargetValueRef, TimeZone},
    path,
    prelude::Bytes,
    value::ObjectMap,
};

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn test_vrl(data: &String, runtime: &mut Runtime, program: &Program) -> VrlValue {
    let mut vrl_value = serde_json::from_slice::<VrlValue>(data.as_bytes()).unwrap();
    let mut target = TargetValueRef {
        value: &mut vrl_value,
        metadata: &mut VrlValue::Object(BTreeMap::new()),
        secrets: &mut VrlSecrets::default(),
    };

    let runtime_res = runtime
        .resolve(&mut target, program, &TimeZone::Local)
        .unwrap();

    if let VrlValue::Object(metadata) = target.metadata {
        metadata.clear();
    }
    runtime.clear();
    runtime_res
}
fn test_rust(data: &String) -> VrlValue {
    let mut vrl_value = serde_json::from_slice::<VrlValue>(data.as_bytes()).unwrap();

    let tag_obj = match vrl_value.get(path!("tags")).unwrap() {
        VrlValue::Array(ref array) => {
            let mut object_map = ObjectMap::new();

            for tag in array {
                if let VrlValue::Bytes(bytes) = tag {
                    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
                        let key = &bytes[..pos];
                        let value = &bytes[pos + 1..];
                        let value = VrlValue::Bytes(Bytes::copy_from_slice(value));
                        // If the key already exists, use an array to store the values
                        object_map
                            .entry(String::from_utf8_lossy(key).into())
                            .and_modify(|e| {
                                if let VrlValue::Array(array) = e {
                                    array.push(value.clone());
                                } else {
                                    *e = VrlValue::Array(vec![
                                        std::mem::replace(e, VrlValue::Null),
                                        value.clone(),
                                    ]);
                                }
                            })
                            .or_insert(value);
                    }
                }
            }

            VrlValue::Object(object_map)
        }
        _ => unimplemented!(), // Return Null if the input is not an Array
    };
    vrl_value.insert(path!("tag"), tag_obj);
    vrl_value
}

fn bench_group(mut runner: InputGroup<String>) {
    // A simple VRL script that sets a `.greeting` field on an object
    let script = r#"

# Create a temporary object to hold our flattened tags.
tag = {}

# Iterate over each string in .tags
for_each(array!(.tags)) -> |_index, t| {
    parts = split!(t, ":", limit: 2)

    key = string!(parts[0])

    current = get!(tag, [key])
    if is_nullish(current){
        tag = set!(value: tag, path: [key], data: parts[1])
    }else if is_string(current){
        tag = set!(value: tag, path: [key], data: [current, parts[1]])
    }else{
        .ok = current
        current, err = array(current)
        if err != null{
            push(current, parts[1])
            tag = set!(value: tag, path: [key], data: current)
        }
    }
}

.tag = tag
"#;

    let functions = vrl::stdlib::all();
    let program = vrl::compiler::compile(script, &functions).unwrap().program;

    runner
        // Trashes the CPU cache between runs
        .add_plugin(CacheTrasher::default())
        // Enables the perf integration. Only on Linux, noop on other OS.
        .add_plugin(PerfCounterPlugin::default());
    // Enables throughput reporting
    //runner.throughput(|input| input.len() * std::mem::size_of::<usize>());
    runner.throughput(|data| data.len() * 1000);
    runner.register("vrl", move |data| {
        let state = RuntimeState::default();
        let mut runtime = Runtime::new(state);
        for _ in 0..1000 {
            black_box(test_vrl(data, &mut runtime, &program));
        }
        // The return value of the function will be reported as the `OutputValue`
    });
    runner.register("rust", move |data| {
        for _ in 0..1000 {
            black_box(test_rust(data));
        }
        // The return value of the function will be reported as the `OutputValue`
    });
    runner.run();
}

fn main() {
    let gen_num_tags = |num_tags: usize, duplicates| -> String {
        assert!(duplicates <= num_tags);
        let mut tags = Vec::new();
        for i in 0..num_tags - duplicates {
            tags.push(format!("env-{}:ec2", i));
        }
        for i in 0..duplicates {
            tags.push(format!("env-{}:ec2", i));
        }
        serde_json::to_string(&json!({"tags" : tags})).unwrap()
    };

    // Tuples of name and data for the inputs
    let data = vec![
        ("1 tag", gen_num_tags(1, 0)),
        ("10 unique tags", gen_num_tags(10, 0)),
        ("50 unique tags", gen_num_tags(50, 0)),
        ("40 unique tags, 10 duplicate", gen_num_tags(40, 10)),
    ];
    bench_group(InputGroup::new_with_inputs(data));
}
