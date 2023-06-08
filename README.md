# RLox

A lox bytecode interpreter written in Rust. Like others on munificents list, a
first project in rust. The result is a middle road between porting all the
tricks of Clox into Rust and using Rust's borrow checker to take work away from
the garbage collector.

## Why?

I wanted to learn how virtual machines work from 'Crafting Interpreters', but
the byte code interpreter is written in C. I don't know C, so while I can
roughly understand what my clox implementation is doing, I wasn't satified. I
ported the project to Typescript, but janked out the garbage collector in that
case, because Deno provides that out of the box. So this prohect was more about
the garbage collector. While Rust is new to me too, I am more interested in
learning and the tooling is better.

## What did you learn?

I was wondered why 'Crafting' has us implement dynamic arrays and hashmaps.
Doesn't C have libraries for that? The garbage collector is a big part of the
answer: memory has to be managed, so why not use the garabge collector all the
way? A library collection won't play along. Counting allocated bytes is a
problem I needed to work around here.

Performance is another argument. The hash table brougth the performance of rlox
closer to clox.

### What did you think of Rust?

It has many features I wished to see in Java and Javascript. Like how all
methods are extension methods, and the enums. Putting the tests with the source
code also feels better than in a parallel directory.

The borrow checker was great help with getting the all the pointer stuff right,
and miri as well.

