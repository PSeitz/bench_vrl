use binggan::InputGroup;


/// Searches for `target` in the slice of tags, returning the index if found.
fn find_text(tags: &[String], target: &str) -> Option<usize> {
    tags.iter().position(|tag| tag == target)
}

/// The benchmark function: each input is a tuple of (iteration_count, Vec of tags).
/// For each input we run the search repeatedly and use the results to measure throughput.
fn bench_search(mut runner: InputGroup<(usize, Vec<String>), u64>) {
    
    // Report throughput as the total number of processed bytes.
    runner.throughput(|input| {
        // Each iteration processes the entire array of tags.
        let total_bytes: usize = input.1.iter().map(|s| s.len()).sum();
        total_bytes
    });

    runner.register("search", |input| {
        let (iterations, tags) = input;
        let target = "target";
        let mut found_acc = 0;
        // Run the search the specified number of times.
        for _ in 0..*iterations {
            // Use black_box to avoid over-optimization.
            if let Some(index) = find_text(tags, target) {
                // Accumulate the found index (just to make use of the result).
                found_acc += index;
            }
        }
        // Return an output value so binggan can report it.
        found_acc as u64
    });

    runner.run();
}
fn main() {
    let mut inputs = Vec::new();

    // Create three variations of tag arrays with unique tags.
    let tags_variations: Vec<Vec<String>> = vec![
        (0..10).map(|i| format!("tag{}", i)).collect(),  // 10 unique tags
        (0..50).map(|i| format!("tag{}", i)).collect(),  // 50 unique tags
        (0..100).map(|i| format!("tag{}", i)).collect(), // 100 unique tags
    ];

    let iterations = vec![100, 500, 5000];

    // Combine each iteration count with each tag variation.
    for &iter_count in &iterations {
        for tags in tags_variations.iter() {
            let name = format!("iterations {}: {} tags", iter_count, tags.len());
            inputs.push((name, (iter_count, tags.clone())));
        }
    }

    bench_search(InputGroup::new_with_inputs(inputs));
}
