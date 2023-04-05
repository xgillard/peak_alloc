# Peak Alloc

Peak Alloc is a dead simple and willingly low overhead allocator for rust 
which allows you to track (and consult) the amount of memory that is being
allocated to your process as well as the *maximum* amount of memory that has
been allocatd to your process over the course of its life.

### Note 1:
When I mean that peak alloc is low overhead, I mean that all it ever maintains,
is a pair of two atomic usize. So the overhead is low..._but there *is* and 
overhead_ because of the atomic number manipulations.

### Note 2: 
The peak allocator is really just a shim around the system allocator. The
bulk of its work is delegated to the system allocator and all `PeakAlloc`
does is to maintain the atomic counters.

## Usage
In your `Cargo.toml`, you should add the following line to your dependencies
section.

```toml
[dependencies]
peak_alloc = "0.2.0"
```

Then in your main code, you will simply use it as shown below:

```rust
use peak_alloc::PeakAlloc;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

fn main() {
	// Do your funky stuff...

	let current_mem = PEAK_ALLOC.current_usage_as_mb();
	println!("This program currently uses {} MB of RAM.", current_mem);
	let peak_mem = PEAK_ALLOC.peak_usage_as_gb();
	println!("The max amount that was used {}", peak_mem);
}
```
