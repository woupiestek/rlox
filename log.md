# rust-lox

## 2023-05-14

### why linked lists?

Crafting uses linked lists as a pattern in many places, but are they needed?
Some of the examples may be cases where hand wrinting the code is conceptually
more simply, given that every datastructure is hand rolled. I.e, reorganizing
everything into an array would be less convenient. The requirement to handle
memory probably figures into this.

I.e. nested compilers, unvalues etc. could be arrays, but that would require
more heap allocation adn garbage collection.

The issue case with the objects is more complex different. For example a Vec
holding all the objects, and garbage collection consisting of moving all the
live objects might be faster than traversing the linkined list, but note:

- The vec could only hold pointers to objects, not the actual structures, since
  moving the actual objects around would break their interconnections.
- The marks will still have to be part of every object, that is to say: the mark
  has to be reachable from the owner of the object, a wrapper with the mark
  attached just takes more space,

### manage what?

Are anything but instances truly dynamic in lox? Functions are defined at
compile time, and could be stored for the duration of the run time, couldn't
they? Same goes for complie time constant strings: no need to manage their
memory, or is there?

What about classes, closures, upvalues, bound_methods? Those could have dynamic
data in them, like from clock. Don't let the lack of input distract you!

Closures are definitely similar to instances, with dymamic data in their
structure. Clox has classes contain closures, instead of functions. It makes
sense because the definition of a class occurs in a context that provides data
to it, acting like static fields. Yet with functions, there is a separation
between the static function, and the closure at run time, so why not with
classes?

Maybe closures should always just contain classes, with functions being a single
method class (following some naming convention):

- function: {name: string?, arity: int, code: chunk}
- bound_method: (instance, closure)
- class: {name:string, upvalue_count: int, methods: function{}}
- closure + instance: (class, up_value[], fields: value{})
- up_value: ??? value

Is a bound method really something separate? Maybe it is another instance, but
funneling it through the same closure + instance structure is a bad idea.

Why do it like this? Many to take work away form the garbage collector: just
manage upvalues and fields, leave the functions and classes alone. the options
of just calling

Perhaps this works:

- class: {name:string, upvalue_count: int, methods: function[], super?: class}
- instance: (class,up_values[],fields: value{})
- closure: (class,up_values[]) with single method class
- bound_method: (class, upvalues[], fields:value{}) with single method class

So the compiler generates hidden classes to account for bound methods and
closures where they show up. Bound_methods seem plenty sensible as extra
structure, though.

A list of methods in the class instead of the hash map... the trouble really is
that even if an object belongs to a class, lox allows shadowing methods with
field of the same name... the compiler doesn not know the index of the method,
if it does not know the class anyway.

### function pointers and constructors

A way out of the conundrum is to copy a function pointer into the object at
construction. The class just has a list of named methods, and pointers to those
are stored as values in every created. This is morally what an object is anyway.
Maybe this is a waste of space for short lived objects, so could there be a way
out?

One thing missing about are special data structures for constructors. These are
available under the class' name, callable like closure, and the devices that
carry the upvalues to the instances. If constructors can call methods, then
those will need to be available to the constructor as well...

So the thought is: a constructor is an instance (since it carries up values),
which has its own class. The constructor could be a kind of prototype to every
instances. I am reinventing the class from above, I guess. Like: if a method is
looking for a member, after considering its connected instance, it moves to the
constructor to see if anything is there, and the constructor can refer to super
class constructors.

- class: {name:string, upvalue_count: int, methods: function[], super?: class}
- instance: (up_values[], fields: value{}, prototype?: instance) // methods in
  the prototype or fields
- closure: (up_values[], fields: value{}, prototype: instance) perhaps a 'call'
  field
- bound_method: (up_values[], fields: value{}, prototype: instance) with single
  special method prototype
- constructor: (up_values[], fields: value{}, prototype: instance) with single
  special method prototype (fields contain pointers to the functions in the
  class, the prototype must have the super prototype in there somewhere... maybe
  'super' is just the field that holds the prototype.)

I think the class construct makes more sense now, as the kind of instance that
is needed to support the constructor including actually carrying the upvalues
etc. Is there still a gain? Yes! functions still don't have to be under the
management of the garbage collector! That is what I am optimising for: let rust
handle memory for the compiler, and use lox strictly for dynamic data.

### my story

Wanted to build a vm, didn't like C, did something in typescript, but without
the garbage collector. Retry it in a more interesting language.

## 2023-05-13

Found one with a HashMap instead of a Vec:
https://github.com/tdp2110/crafting-interpreters-rs/tree/trunk

### Loxido

The guty write an entire blog on his experiences:
http://smallcultfollowing.com/babysteps/blog/2018/11/01/after-nll-interprocedural-conflicts/
Basically, he offer two implementations, one based on Vec, one with Unsafe
pointers.

Pointers to https://rust-unofficial.github.io/too-many-lists, which explains
parts of the problems I am trying to solve.

### internings strings

Straightforwad interpreter combines a Vec with a hashmap, sacrificing some space
for simplicity. Strings become `usize`. No garbage collection as far as I can
tell: this would screw up the data structure. Hypothetically, the same can be
fone for (native) functions, and maybe upvalues.

I seem to have solved the string pool issues somehow. Some testing is necessary
of course.

What do I rreally want to learn? The garbage collector itself is more important
than the details of hash maps ans sets etc. So couldn't that work?

I guess I'd like to replace the linked lists as well, because that is along the
same lines. Idk what that does to garbage collection: just moving all values to
a new vec doesn't sound performant, but maybe it is quicker than running though
a linked list.

IDK, I think I am going to be pretty happy with any performance--my goals is the
big picture and choice of datastructure for best performance is a detail.

### finishing review

- One of the few unsafe ones: https://github.com/Kangaxx-0/rox

- A jit compiler for x64 machines: https://github.com/miDeb/loxjit/tree/main

- Rust Zig comparison: https://zackoverflow.dev/writing/unsafe-rust-vs-zig,
  https://github.com/zackradisic/rust-vs-zig. I think I saw thios on the
  primagen.

So the choice is unsafe versus slow implementations based on Vec. And maybe Zig
would have been a beter choice.

## 2023-05-12

Interesting, here everything is `usize`:
https://github.com/rctcwyvrn/rlox/blob/master/src/value.rs#L228. Is related to
using a vec to keep track of all instances. This is as far as I got today. This
implementation suggests that the garbage collector can be limited to closures
and classes, using rusts memory safety for the rest.

Clox uses a single linked list of everything allocated, and pointers, of course,
instead of indices into an array.

### runtime data model

- bool
- nil
- number
- string
- native_function
- function: (string?, int, int, chunk)
- bound_method: (instance, closure)
- class: (string, closure{})
- closure: (function, up_value[])
- instance: (class, value{})
- upvalue: ...value...

## 2023-04-11

Options for the garbage collector:

- all unsafe, with *const or *mut
- admbiltfcliff like, with Rc<> and Weak<>

## 2023-05-10

### Implementing memory

I think everything hinges on this: what does the heap with all the values look
like?

- just working with `&` meant lifetimes all over the place. Maybe there is a
  solution there, but maybe I am walking into a trap here.
- I now rely on box, because it is called heap allocated, but I doubt that will
  be much on an improvement.
- I guess using raw pointers like the C example will be an endless fight with
  the compiler and unsafe code, in which case: why is doing this in rust better?
- I see this SlotMap based interpretation: it replaces pointers with keys into
  this map.
- Could be interesting:
  https://github.com/adambiltcliffe/rlox/blob/main/src/value.rs#L202
- I should just go through the list here, and note any interesting option:
  https://github.com/munificent/craftinginterpreters/wiki/Lox-implementations

## 2023-05-07

The clox scanner uses pointer into the char array, which can be substracted form
one another to track progress. The tslox one canot accss the pointers, so it
uses indices instead. Rlox has utf8 strings, however, so we cannot decently to
either: a character counter is needed to get correct column numbers, but only
for correct column numbers!

Good reminder: string[..3] is three bytes, not three characters. So... shit?
`self.as_bytes()[usize]` `byte.is_utf8_char_boundary` ~ it is not all terrible.

One more idea: tokenize the bytes, since this is good enough for lox anyway.

## 2023-05-06

Learning rust while regurgitating crafting interpreters.

### the scanner

Some ideas: probabaly best to use the iterator over the chars to create an
iterator of tokens. Do the 4 fields of the struct still make sense? In
typescript it becomes 5, but uses indices, which don't work in rust.
