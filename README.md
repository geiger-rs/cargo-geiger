# cargo-geiger

A program to list unsafe code in a Rust project.

This project is in its current state a quick-n-dirty, glued together, remix of
two other cargo plugin projects:
<https://github.com/icefoxen/cargo-osha> and
<https://github.com/sfackler/cargo-tree>.


# Usage

1. `cargo install cargo-geiger`
2. Navigate to the same directory as the Cargo.toml you want to analyze.
3. `cargo geiger`
4. Please don't look at the `--help` flags, they are inherited from cargo-tree
   and may not work as intended. TODO: Review and update command line flags.

# Output example:
```
Compact unsafe info: (functions, expressions, impls, traits, methods)

cargo-geiger v0.1.0 (file:///Users/u/code/cargo-geiger) (0, 0, 0, 0, 0) 
├── cargo v0.27.0 (3, 148, 2, 0, 4) ☢
│   ├── atty v0.2.10 (2, 8, 0, 0, 0) ☢
│   │   └── libc v0.2.40 (0, 0, 0, 0, 0) 
│   ├── clap v2.31.2 (0, 1, 0, 0, 0) ☢
│   │   ├── ansi_term v0.11.0 (0, 23, 0, 0, 0) ☢
│   │   ├── atty v0.2.10 (2, 8, 0, 0, 0) ☢
│   │   ├── bitflags v1.0.3 (0, 0, 0, 0, 0) 
│   │   ├── strsim v0.7.0 (0, 0, 0, 0, 0) 
│   │   ├── textwrap v0.9.0 (0, 0, 0, 0, 0) 
│   │   │   └── unicode-width v0.1.4 (0, 0, 0, 0, 0) 
│   │   ├── unicode-width v0.1.4 (0, 0, 0, 0, 0) 
│   │   └── vec_map v0.8.1 (0, 0, 0, 0, 0) 
│   ├── core-foundation v0.5.1 (0, 530, 2, 1, 13) ☢
│   │   ├── core-foundation-sys v0.5.1 (0, 0, 0, 0, 2) ☢
│   │   │   └── libc v0.2.40 (0, 0, 0, 0, 0) 
│   │   └── libc v0.2.40 (0, 0, 0, 0, 0) 
│   ├── crates-io v0.16.0 (0, 0, 0, 0, 0) 
│   │   ├── curl v0.4.12 (4, 598, 5, 0, 1) ☢
```

# Why the name?

<https://en.wikipedia.org/wiki/Geiger_counter>

Unsafe Rust and ionizing radiation have something incommon, they are both
inevitable in some situations, but should preferably be safely contained!


# On Unsafe (originally from the cargo-osha README):

> Number of lines of code inside the unsafe blocks themselves isn't a useful estimate.
> 
> -- /u/kibwen

> I don't think any sort of counting can help.
> 
> -- /u/DGolubets

> Unsafe isn't your enemy.  Unsafe is your friend... you know,
> the friend who lives in the country and has a giant pickup truck and
> 37 guns.  You might not bring him to your sister's wedding, but if
> you need something blown up, he is THERE for you.
> 
> -- icefoxen

This tool isn't intended to "measure unsafety", it's intended to be a quick
and dirty investigation, because partial information is better than no
information.  `/u/annodomini` on Reddit said it best:

> It's a quick metric for doing a preliminary overview, not a replacement for doing proper auditing.  
> 
> Taking a look at the output of cargo-osha posted elsewhere in the thread, there are 1025 unsafe expressions in actix-web, out of a total of 37602. That tells me that there's a pretty large auditing job to do, just to determine what invariants need to apply for those 1025 unsafe expressions to be valid, let alone auditing any of the code within privacy boundaries that the unsafe code relies on to uphold those invariants.
> 
> If a crate has two or three unsafe expressions, that tells me that it could be relatively quick to audit those expressions, figure out the invariants they rely on, and then audit everything in the code base that could impact those invariants; for instance, if it relies on invariants about length and capacity, then from there you just have to audit things that touch length and capacity. Or in some cases, the unsafe code doesn't really depend on any other invariants, and can be audited in isolation.
> 
> On the other hand, if you have 1025 unsafe expressions, that's a huge job to audit all of those plus everything that could impact whether those are actually valid or not.

