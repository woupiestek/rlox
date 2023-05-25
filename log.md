# Rlox

## 2023-05-25

Main has stack overflow on 'hello world'. Big frames or endless loop?

## 2023-05-24

### driven nuts

So in clox compiler, a string is a pointer and a length. Rust allows that as
slice, but requires that we track the life time. This is killing. Indexing into
the source string works until synthetic token pop up, which index into other
strings.

Got it, I think. I didn't use generic lifetime parameters in the impls. This
made working with types with lifetypes attached impossible. Even after I
corrected that, I still menages to get strange error because the lifteimes
needed to be in retrun variables.

This get me thinking about the gabrage collector: We could still go for a
solution where every managed reference is a borrow from the heap. The heap could
still deallocate, but perhaps this gives a little more control?

### reverse marking

So on dropping handles, objects could be marked as potnetially garbage. As the
collection cycle begins, the collector assumes not dropped means rooted. This
leads cycles never getting cleaned up.

Trace through teh object for potential garbage, then assume waht is left over
are roots, then trace again. This is probabaly really bad for performance.

### one stack of locals

There is only one stack, so does the compiler really need multiple collections
of locals? Maybe we can alllocate all the spaece at once! And the upvalues...

So basically, have one big vec with all the locals in it

- each 'compiler' has an offset in a shared locals vec
- begin_scope stores offsets, and end_scope resets them, while ensuring that
  bytes are emitted as they should. No depth in local variable needed
- add_local just pushes on top
- mark_initialized: could that not just be a status, like is captured?
- declare_varibale just looks for the name in current scope, as determined by
  offset... is this a bad system?
- resolve upvalue: the hard part may be getting all the upvalues right. putting
  data with the variables might be helpful: which 'compiler' and 'scope' do they
  belong to? but with indices to those tables.

Interesting data to add: types of variables on the stack!

Whether there are similar options with upvalues is a mystery to me.

## 2023-05-22

### growing understanding

Things are getting allowcated on the stack or heap as intended.

### variable things

There is some ceremony around the names, required for variables to be resolved
correctly.

### super class

So note what needs to happen. The compiler sees the name of the superclass but
nothing else Classes must therefore be linked up at runtime! this requires:

1 getting the superclass on the stack 2 getting the superclass added to the
object we are creating

Under the name of the class, what gets in scope is an initializer. So 'super'
links up these initializers instead. Now super will be a value, since it lives
on the stack, though it should be a value of init type.

Is class still worth it?

- name: just a local variable refering to init, or not? it may be needed for
  print and stringification.
- up_value_count: relevant
- super_class: seem pointless now: the compiler doesn't know what it is
- methods: though one. A vec of methods is not as useful as a hashmap of one.

- constants: also relevant

At the site of the class declaration, what is actually constructed is an
initializer. It is distinct from a bound method, because an initializer has no
instance.

Theoretically the initializer can constructed many times off the template that
the class provides. The class and the methods are not created from scratch at
that point.

The methods though... Remember how the get copied over every time. There is no
traversal, except maybe for super calls, which are resolved by keeping the super
init on the stack.

Okay, so this is where the upvalue stuff breaks: inherit a method, and its
upvalues are still part of the initializer of the superclass. Is that a problem
now? the compile time method just refers to upvalue n, but at run time, the
method may run ondar a new initializer with a new set of upvalues.

Why isn't this a problem for the constants though? Because those are attached to
the closure objects and copied over as well.

### deminishing distance

I feel like giving up on my ideas and just sticking to the clox model now. break
up the class object and spread its contents. There may be a point to sharing
constants between methods, but that means methods must keep a reference to their
classes, which potentially slows things down. Also, the change of overflowing
the constant table, that only allows 256 entries, become much bigger.

For the upvalues: I am unsure that sharing between methods of a class would do
much good. And the initializer thing... the methods would need to link to their
initializer for the upvalues, and maybe the constants a extra step, it seems, in
their execution.

### resistance

Let all the methods defined in the same scope share an upvalue object, if not a
link to the closure that birthed them. It feels like it could be more
efficient...

Then again, I don't get upvalues well enough. Let's just refactor to be closer
to the original. Keep these ideas for some other time.

### methods and classes

## 2023-05-21

### some parsin'

I want to use the rust error handling system. The idea would be to propagate any
error up to `declaration`, at which point it is handled by synchronize. but I
run into the response of `advance` to error tokens: it puts the parser back in
panic mode. Why though?

Let's work this out later.

### splitting between classes and methods

I reinterpret a closure as the bound method of a single method class. So they
are syntactic sugar. Even when compiling a function, you are still inside a
rudimentary class. This should even apply at the top level. The main difference
is that constants and upvalues are collected at the class level.

During compilation clox uses a stacks of compiler classes to keep track of
context data I did the same, but split these up as well. Maybe I moved to
fast...

It is possible to be in a class declaration without being in a method, but it is
not very useful. So this could be a special case, where no compiler struct is
generated. So, I can merge the structures!

### parsing expressions

So lox has 36 opcodes, which suggest room to add many more. I'd like to have a
lot of compare and jump operators for interpretation. Is that difficult to
implement?

### matter of organisation

It seems attractive to attach more methods to the compiler struct, because the
compiler module is such a beast.

### some ideas

Don't have the methods in the class or the compiler, just the typed handles.
Perhaps that also removes some of the struggles I have had with Rust: the fact
that I want to take part of the data structure and change it, isn't accepted.

Garbage is created by the compiler, every time a vec is resized to contain more
elements. Hence the need to run the garbage collector at that time as well.

This changes a lot of things...

The top level script will now be an empty name method. This is because we force
it into some virtual class. Maybe there is a workaround... the runtime could
generate a new name, for each query. as long as it is no valid identifier-- e.g.
a number--it should never lead to conflicts.

## 2023-05-20

### roots

The things clox garbage collector keeps track of:

- functions in the compiler (static)
- operand stack (dynamic)
- closures in stack frames (static)
- upvalue stack (dynamic)
- globals table (dynamic)
- initstring (static)

It does not track the stringpool, but makes sure to remove the elements before
they are dereferenced.

### dynamic typing

There are kinds for every type in this world, and I keep thinking these should
just be like classes, carrying a number of methods and so on, so no mathined
needs to happen, but this doesn't fit rust to well.

- turn kind into a trait. once again, do

### recover value from handle

it feels like each kind should have a service to take care of this.

for a lot of stuff i just want oop like semantics: there is an instance in
storage, and the pointer points there, until the object is cleaned up.

Maybe I think too much about how to do this in java or scala... The kinds would
be objects with their own build, destroy & trace methods

How to link these up, btw?

- build: type dependent
- detroy: potentially value dependent
- trace: somewhere in the middle I guess

Note that kind only takes up one byte, and the boolean next to it another. We
could even make that tighter. So how about explicit vtables?

i mean, the memory manager need the layout and the trace function

Ideas:

- add a header to each allocation (note that due to alignment, the header is
  typically 8 bytes long, so there is no point in saving space by combining the
  is_marker boolean and the )
- have a static table of kinds, with layouts and trace methods.
- the layouts may not even be that useful ~
- use an index into this table
- there is a global list of kinds

The garbage collector need to know the type when allocating and dropping managed
objects. Those types are recorded in the object headers. Now we just need a
mapping from these reifications back to the types. How? What is efficient?

Dispatch needs to happen somehow. I just want it to be correct, ergonomic and
fast. Bonus if the indices are automatically correct.

Maybe a kinds manager can take care that the indices are correct.

A lot of effort here, and it may all be in vain, because we need to cast to
specific types in other contexts, and we need the kinds for that as well. i.e.
is_constructor, how is that going to work?

Kind can still be u8, but the u8 needs to show up more often. Like we have a
generic method somewhere to do all that work:
`Kinder::register::<T:Traceable> {}` To wrap a value now requires this id again,
though. Ok, don't register the ids, put them in the Traceble trait, perhaps
check that there are no duplicates.

Dispatch is unavoidable, since we need to map the reified types back to their
originals, with .is_<type> and .as_<type> methods. maybe we can do that with a
table lookup once, and just generate the code for each type.

Where would be put this? `register!(kind, Type, is_type, as_type)`

- the handle needs some of these methods,
- Traceable works on `Type` itself, but is needs the same kind,

### reconsidering the linked list

Consider that building the vec of live objects may mean allocating space of that
vec first--potentially reallocating as it grows--and releasing the space of the
old vec afterward. Simple approach, but possibly time consuming?

Alternatives:

- look for a live object from the top down; then look for a dead object from the
  bottom up; swap; repeat.
- look for dead objects from the bottom, then look for the next live one, swap,
  and repeat;

I think the first one may slow down the cache, while the second one may do more
swaps, since some dead objects will be swapped multiple times. I don't know when
the cache gets involved to slow up down.

The links in the linked list don't have to be moved at all, so that is a plus
for clox.

### compile time data

It seems like after the compiler is done, the structures can be borrowed
indefinitely,until the machine quits. Even the commandline version should be
able to live with append only semantics. So can't we just build it like that?

options:

- allocate extra space on the heap to copy compile time structures
- do something complicated with borrows: the shared lifetime of the
- build a second heap, with compile time data, and the semantics above

### strategy

Ordinarily storing an object should check if garbage collection, but that can
only happen if we know all the roots. Alternative: let the heap refuse to store
new objects if it is at capacity. Just panic! This forces the caller to go
through the effort of increasing capacity and triggering garbage collection. So
we arive at the problem of measuring how many bytes are allocated. And here we
see the true reason why clox needs to define its own dynamic arrays and
hashmaps: it is not possible to say how may bytes are allocated, if
reallocations happen in hashmaps and vecs that aren't tracked by the vm.

options:

- use the number of objects allocated as proxy, or use some other threshold;
- implement dynamic arrays and tables, as in clox; perhaps add an allocator
  class, for good measure;

Keeping track of every allocated byte seems really careful. How could you trick
the garbage collector into actually running out of memory with garbage? If there
is an object number trigger, then creating lots of objects is out of the
question, but filling an object with values maybe? A regular OOM is unfortunate,
but not what we are defending against. So have do the values become garbage? It
feel like: as soons as an object is garbage, then there is nothing that could
make it grow further. So make a few really big objects, by flooding their field
tables with assignments (to different fields), and discard them.

Other allocations are more predicatible, but could be an issue just the same. I
imagine thats we could count certain instructions, and just garbage collect
whenever a certain number is exceeded.

### decisions

To speed up development:

- use same garbage collector for compile time date
- just use string.
- don't have a byte counting garbage collector

A string pool could still be used during compilation to ensure that keys are
always the same string, but it would be the hashset version, and be deallocated
after compilation.

## 2023-05-19

To gc or not to gc? I was thinking the compiler doesn't need garbage collection,
since bytecode, constants etc. last forever. But how does this work on the
commandline? How do we manage memory otherwise?

Maybe the clox example is just the simplest solution, and I should just go for
that.

What happens to the vecs an hashmaps placed on the heap?

The remove from heap ensure that we properly deallocate the subobjects.

We are left hanging somewhere in the middle, are we not?

### trait object for management

Maybe it is better for any heap managed entity to have a trait for the heap
allocation:

- move to heap
- remove from heap
- mark & trace

All that is needed is a function to fetch the trait object belonging to the
managed type.

### lifetimes

So that could be another solution: always borrow objects form the heap,
lifetimes and all. That seems too complicated, though. Maybe it a better to
somehow change the state of handles to indicate that they have been garbage
collected, so dangling pointers become runtime errors.

### embedded vecs and hashmaps

So this is my concern: the vecs and hasmaps have data allocated on the heap out
of sight of my garbage collector. When garbage is collected, the pointers to
that data are lost, but the data remains.

I try to move it back out, but the borrow checker won't let it happen.

I can:

- create my own associative array and hashmap, as shown in clox
- figure out some solution to restore the embedded structures.

To put it in sharper perspective: I do not know how Vecs and HashMaps allocate
and deallocate, and therefore cannot manage their memory. `

gains from self implementation: 1 use generics for type safety 2 don't manage
memory that doesn't need it.

Such a big project, and almost no running code yet.

Note: this may be another reason to implement datastructures from scratch: even
if C has nice hashtables, they would not necessarily play nice with the garbage
collector.

Another note: maybe clone and copy are not so bad...?

### managing compile time data

For a file, the compiler can just deliver the result and be done, but the cli
has repeated runs, that can at least shadow classes and methods with new ones.

Originally I intended to separate storage, but is that wise?

### structure

In clox everything seems to depend on everything. That drives me nuts.

## 2023-05-18

### stacking

The stack seems like a fine data structure to introduce and reuse. Should it be
allocated on the heap? I have no idea what helps with caching. What about the
elements? It seems that elements should just be copied in and out, but the stack
frames refer to methods generated by the compiler and upvalues captured by the
environment.

Wait, the upvalues are basically on the stack now, through the constructor of
the instance. It may be distant, but we don't need a seperate pointer there.
This leaves the matter of the methods.

Perhaps method handles are the right abstraction, even though methods are not in
managed memory, and there is no need for mutability.

### pointers

I am now runnign into the problem of poining to methods and classes generated by
the compiler. I didn't want managed memory, but this may now have complicated
things.

It still seems like classes, methods and Strings don't need garbage collection,
but I need a way to point to them from unmanaged memory.

There is a lot is still have to leanr bout rust, and none of the code is
actually functional now. But let's continue, and make changes wherever needed.

### heaping

And yet another kink: what does the heap contain? `Vec<Box<?>>`! The type
punning approach looks scary, but I see it solves a problem.

I guess I need a static memory in the end: the 'class path', because of the
split I made. Was deviating form the clox example a good idea?

### powers combining

Rust calls drop, but this is takes as a signal by the garbage collector that an
object potentially is garbage. Only at that point, the object becomes managed.
It is too soon to think about such optimisations.

### remaining memory

- class path
- string pool
- globals
-

### heaping strategies

I put the objects in a vec, because i don't not want a linked list. sweep now
means popping handles from one vec into another. This has nothing to do with
layout of course.

One idea: instead of marking objects, just register them again. no: we'd have no
idea whcih object are ready to go.

### gray matter

instead of recursively going through the graph, collect all the marked object in
the gray stack, and process their references from there: this prevents stack
overflow.

Like this: the current vec is white, gray is a middle vec, handles move in and
out ending up in the third black vec. this becomes the new white vec, as the old
one is discarded, destroying some handles, and unmarking others.

## 2023-05-15

### changing things up

Make a distinction between static and duynamic strings, then note that hashmaps
thoughout lox are only keyed with static strings, or aren't they? There is a
cross over from static to dynamic with string literals, but that is all. The
dynamic strings don't need the interning or the hash codes, The static ones
don't need memory management. As far as dynamic strings go, can we do more than
print them or add them up?

Use the enums for the objects. The main concern is wasting space now, which clos
doesn't do, can it be prevented? Rust enums will take up as much spacer as their
greatest member plus extra bytes for telling apart. Many members is therefore
good. I've designed the types first, and may have to change them later, but we
will see.

For nan boxing, if needed: there are 51 bits and 48 bit pointers, we could have
7 flavors of pointer, then, needing the 8th for `nil`, `true` and `false`, and
maybe some other values. Fortunately, I now only need 6! This is after reducing
closures to the tripled of constructor instance and bound method: the closure is
a slimmed down version of the latter.

## 2023-05-14

### why linked lists?

Crafting uses linked lists as a pattern in many places, but are they needed?
Some of the examples may be cases where hand wrinting the code is conceptually
more simply, given that every datastructure is hand rolled. I.e, reorganizing
everything into an array would be less convenient. The requirement to handle
memory probably figures into this.

I.e. nested compilers, unvalues etc. could be arrays, but that would require
more heap allocation and garbage collection.

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

### considerations

The runtime structure becomes different, with no functions in managed memory.

- the class declaration is reinterpreted as the construction of an object that
  contains methods, and the actual constructor.
- instantiation links to the constructor instance instead of the class.
- the bound method is roughly the same, though, just another instance pointing
  to the object and method
- finally closures: they might get hidden classes to hold a single method.

I am unsure how the upvalue stuff works in clox, but the data structure suggest
that they are linked to individual members of a class, rather than to the class.
This implies duplication: it is the class that closes over those variables, and
the methods are part of the class. This translates into the following variation:

- class: (upvalues[], functions{})
- closure: (upvalues[], function)
- instance: (class, values{})
- bound_method: (instance, function)

Is there something about the semantics of Lox i've misunderstood? How could a
method ever be more than function? At least with the bound method, one can argue
that the chain is needlessly long, but then the solution is to With the class,
maybe that is my mistake... though I miss where the upvalues are otherwise.

A simple test seems to demonstrate that methods can close over variables.

### what actually happens

So all functions are stored as constants in chunks, and only the byte that
points to the constant actually makes to to the stack.

Okay, the test seems to show that method can close over variables, but the data
structure show classes don't. So it looks like one way or another, we are
attaching upvalues to individual methods which are then added to classes as
closures. let's inspect method calls. Yes, the vm casts methods to closures upon
calling.

### bound methods again

Why aren't they just pairs of objects and method names? I guess pulling out the
closure is just faster.

### reconsidering chunks etc.

So the chunk contain the code of a single function, which is also the only place
where the upvalue count is stored. I think that explains it.

We could use a data structure that is more like a class, and add
anonymous/eponymous classes where closures show up. It would is interesting to
consider sharing a constant table between all methods, though maybe the methods
ought to be in the constant table.

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
