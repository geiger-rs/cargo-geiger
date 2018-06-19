# cargo-osha

A Cargo plugin to list unsafe code in a Rust project.  Right now
mostly a proof of concept.

Not actually a cargo plugin yet.

# Example

Example from `winit`, a crate that does a large amount of
platform-specific FFI:

```
> cargo run -- winit/src/**.rs
...
Unsafe functions: 20/85
Unsafe expressions: 222/16847
Unsafe traits: 0/14
Unsafe methods: 14/460
Unsafe impls: 21/136
```

Example from `ggez`, a crate that mostly uses dependencies that
provide safe wrappers, and so needs little unsafe code itself:

```
> cargo run -- ggez/src/**.rs
...
Unsafe functions: 0/101
Unsafe expressions: 0/6786
Unsafe traits: 0/7
Unsafe methods: 0/312
Unsafe impls: 1/122
```

# Usage

Right now it only provides a simple command line program that reads in
N Rust source files.  Doesn't walk dependencies or anything, you have
to provide all files on the command line.  It has no command line
flags worth speaking of.

The code itself is nigh trivial, just read the source.  It uses `syn`
to walk a Rust source code file and counts up the occurances of
`unsafe` in various places.  There may be some it misses, just open an
issue or whatever.

Should be split into a library someday maybe.

# On Unsafe

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
> -- me

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

# Why the name?

In the USA, OSHA (Operational Safety and Health Administration) is the part of the government responsible for workplace safety.  It's their job to make sure that construction workers wear hard hats, miners don't breathe asbestos, factory workers aren't crushed by machinery, and all that sort of useful stuff.  OSHA has an impossible job, because if they did it perfectly then nothing would ever get done, but the world is still a much better place because they exist.

Apologies for the obscure name to the rest of the world!
