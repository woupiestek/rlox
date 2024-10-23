# Rlox

## 2024-10-19

### nightly thoughts on garbage collection

Look at the values: when the owner of a value is live, so is the value. At the
first pass that a value is marked as reachable, its members may still require
marks, so those _rows_ are marked grey. When a grey row is encountered, it can
be marked black, and subtracted from the grey count. When the grey count
vanishes all rows and objects are marked.

### unknown signature

The arrays can only be 75% full, which is hard to control when the number of
properties for each object can vary wildly. It seems like doubling the size of
memory always means half the objects must move to new memory, while the other
half must be compacted. Maybe there is another way to do search, however. Start
linearly from an index in the first array, then from the same index in the
second then from the same index in the third and so on. New pairs first fill the
tombstones, but can only be inserted in the last array, since only the last can
actually fill new slots.

### space requirement

Per property, space is required for the value, the key, and the owner handle,
plus at least one third for linear search to work. The owner handle is needed to
confirm that the right index is found. It is better if each property has its own
column then, but only if all columns are always in use.

### the sum of all classes

Let the `object` type be the dependent sum of all classes: not a class itself,
in other words.

## 2024-10-18

### property or method resolution

Normally the object handle is a pointer and the properties are keys to the
content. For data orientation, this gets turned around: each propterty is a
collection that maps object handles to values. The handles are essentially just
numbers.

I imagine each property could be a hash map, but this seems imply a little
duplication: a hash map has a mapping to indices into an array of values. The
hash set part is needed to avoid hash collisions.

So that is one use of classes: it is literally a set of all live objects and the
mapping to the indices into property arrays. Classes also keep track of all
properties for the purpose of garbage collection, and the methods.

Instead of giving each object an index, the class could also provide indices for
pairs of objects and properties. In this case the list of properties in an
object needs to be tracked for garbage collection purposes.

The subdivision into classes may not strictly be necessary: just one big hashmap
that takes property object pairs to values. Or perhaps the subdivision should
not even be class based. The space is 75% full at a certain number of objects,
depending on the number of properties, and we don't have to move everything
around.

### garbage collection

There must be a collection of grey objects, in this case ideally a set, so run
down the entire array to create the next set of grey objects, then blacken the
current list it is necessary to keep track of which objects are traced already,
hence the grey ones. Finally make a free list out of the rest. Running down the
lsit like this

Wait, rather than trace objects, trace fields. eventually objects have to be
freed, but by noting which fields have been traced, double work can more easily
be prevented. A set of objects and a set of indices: the indices are skipped and
the objects are followed. Gray is still needed though, not all indices will be
blackened after all.

Grey indices. This because a list of object indices won't cut it anymore.

## 2024-10-17

### even more data oriented VM

Each of the properties of the objects would each get a global array of values,
with a function to map instance handles to ther proper indices. Whether this can
be done efficiently is still the question, but there should be some advantages
when huge numbers of objects get processed. Also, think of what the language can
do that way: all members of a class can gain a field dynamically.

Garbage collection is another story... there is an easy listing of all members
of an object that the garbage collector can use for tracing in the current
layout, which is not so obvious in this set up. What about reference counting?
relies on the same thing I guess: how else to decrement the members of freed
object?

So maybe that is the thing: there should be a seperate service for tracing, that
records dependencies between objects. A list of fields in use, or just the
values. I think the field names in case of mutable objects.

### modules

Much of this only helps when there are many objects of each class. Otherwise, a
module would in run times be a collections of tables, containing the properties
of each type. The module can selectivley export columns and other can
selectively add them.

### dependent sums

It is possible to have incomplete columns, whoch takes care of optional values
and types with different sizes. It is no longer a waste--in the heap it does not
have to be at least--if nullable columns are stored without space.

## 2024-10-13

### one big hashmap

Would it be bad to combine class and method names, and stuff them all is a
single big hashmap? Can that be done with properties?

Why have class handles? Aren't names just as good? I don't see errors happening
with redeclared classes, but there is a difference in interpretation, of course.
In the nonimal case, the set of methods for existing objects changes at the
point of redeclaration, otherwise the new methods only apply in newly
constructed objects.

So the idea is to combine the class handle and method handle into a single big
hash, so all methods are sotred into a single table. Would this be slower?
Garbage collection might be, if there is no way to collect all methods that
belonged to a class.

Now this idea might apply to intances as well, and who knows, to closures? For
closures, it would combine the closure handle and the upvalue index. For
instances, the instance handle and the property name.

## 2024-09-11

### two ideas

The point is higher efficiency for certain tiny objects, mainly the closures.
The first is that if the data can easily fit in 64 bits, then allocating space
and using a pointer dies not make much sense. So split between small and large,
and use pointers only in the large case. The second is changing the way the data
is stored. 64 bits is 8 times 8, and could therefore represent and array of 8
indices below 256. Such small indices won't be uncommon.

The question is mainly where to do all this. The closure handle has little
space, but if we can fit a function pointer and one or two upvalues in, then the
actual pointers can be used for anything that does not fit.

### options

- use a union of `Box<[u32]>` and `[u8;8]` inside closures, or something like
  that
- turn the closure handle into an array, e.g. `[u8;6]`

Dealing with lots of small but varied size objects is frustrating.

As I see it, the larger porblem of lots of small objects in not resolved, and it
may even have gotten worse. I found no better idea than segregating by size.

### further subdivisions

`0x4000_0000` for small closures, instead of a pointer, [u32;2] is stored for up
to 2 upvalues. Alternative: [u8;8] with variable length number encodings, but
this means some unlucky functions and upvalues will wind up with a pointer. What
is a bigger problem: whether small or large is not determined by the upvalue
count alone, but also by the specific sizes fo the function and upvalue handles,
which leads to much more complicated code for creating the closures.

## 2024-09-10

### closure trick

Boxes are 8 byte pointers, which can store two upvalues. So make that switch:
for closures with 0, 1 or 2 upvalues use the same space to store the upvalue
handles directly. Only use pointers for at least three upvalues. It may still be
worth considering other solutions for the lower arities.

Perhaps it is on the upvalues themselves as well: if the handles are small
numbers, an array of them can be encoded with something like the midi encoding,
and it could help if the differences are small.

This is something: because rlox uses handles for upvalues, and these handles are
likely small numbers, multiple can fit into a u32 or u64. Only arrays with many
upvalues with high handles need extra storage, for which additional space could
be allocated.

The basis was the switch from a pointer to direct storage of values when the
upvalue count is low. The midi encoding idea is secondary.

We need: function handle and a sequence of upvalue handles. No reason to waste
32 bit on a function if they typically only need 16. We just come up with a
structure can store: functions handle, upvalue count, and either a few upvalues,
or an indicator of where values are stored.

## 2024-09-06

### optimizing current pools

Aassuming that each table has a column of at least 4 byte size, once of the
columns could play the role of the keyset and keep track of free space for the
rest. The 4 byte size lets the storage place free handles beyond the end of the
allocated range. Then when the allocated range catches up to used range, new
handles are created. Like having a 'handles pool' as part of every pool, perhaps
with all the logic for mark & sweep attached.

Upvalues are the exception here, as the are pointers to values, but values are
big enough, and a conversion to and from upvalues is easily added.

Eliminating independent free lists seems to make rlox a little slower, except
where garbage collection actually happens. Odd.

- binary_trees: 3.8152577877044678
- equality: loop 5.290778398513794 elapsed 5.792621612548828 equals
  0.5018432140350342
- fib: 2.6560192108154297
- instantiation: 0.9986624717712402
- invocation: 0.6791610717773438
- method_call: 0.47519421577453613
- properties: 0.9306488037109375
- string_equality: loop 1.5972368717193604 elapsed 1.7262482643127441 equals
  0.1290113925933838
- trees: 7.590463161468506
- zoo_batch: 10.002305746078491
- zoo: 0.7005958557128906

### buddy allocator

Since maps and keysets use 2 \*\* (i + 3) amount of space to store data, so this
might be an opportunity to use a buddy allocator.

How is free space managed?

A free list for each power perhaps. If one of the smaller has non empty, find
one bigger. A free heap, with priority by what? Largest power at the root, since
there a relatively few. Or smallest at the root, since these are needed most
often? Finding the best fitting is hard either way. Free list can be implemented
as linked list with links inside the memory. At least, one always works with the
free list, instead of elements at the end.

Note: allocation each power of object happens with a specific alignment. Perhaps
aligning by own size is a space saving mechanism are all sizes... No, I don't
believe it is.

The trouble her is that I don't knwo if what I am doing is any smarter than the
standard library already provides. especially as space requirements grow, it
seems an extra layer of indirection is needed, to allow for multiple base
pointers, or a lot of code has to be moved around. Maybe try this after figuring
out profiling.

### small object pooling

This is mainly about the closures: many are mere functions, most of the rest
will have a small number of upvalues. This idea is that beyond a threshold, the
'allocator' requests dedicated space for these upvalues, but the smaller cases
are pooled by size. Using buddy allocation more sizes can be supported. One can
reduce waste by having buddy allocators for small odd numbers, so for larger odd
sized objects less rounding up is needed to fit them in one. Lots to tune here.

My first idea was not to use buddies but just slabs dedicated to a size. If one
of the slabs comes free it could be repurposed for storing differently sized
objects. Is that something to build for? A slab could store two sizes by
starting from opposite ends, to waste less space, especially if pairings of
relatively large and small are made.

The handles would break up into a subhandle for the slab and an offset within
each slab. This could say something about the boundary between large and small.
e.g. of offset in the slab must be a byte, so at most 255. To waste at most 5%
when full the element needs to be about 10% of the size of the slab, e.g. 25
bytes. Now pair sizes: 1 & 25, 2 & 24, 3 & 23 and so on. Perhaps the very lowest
should be special cases if that gives faster access.

Because complementary sizes are pared, we can just take 25 & 24 as starting
point, and work down to 48 & 1, 48 being the threshold for largeness.

### data oriented abstract syntax trees

For a project that uses them: pool the nodes by type. Not sure if I see the
benefit here yet, except... strong typing while locally using bump allocators.
Operations specific to a type of node don't have to search the whole tree for
them. The type checker can do the same thing: pool by type constructor.

## 2024-09-06

### refactoring string garbage collection

...and fixing a number of bugs

## 2024-09-05

### debug & benchmark

- binary_trees: 4.429989576339722
- trees: 8.258971929550171

The bugs are fixed and the results form the garbaeg collected tests are quite
good.

### compiling with many stacks.

It remains a question o/c. Tighter packing is nice, but groups of 4 or 8 should
eliminate much waste, while keeping data close together.

For functions: each has a name, code, constants, line numbers But these are to
be replaces with offsets and maybe lengths in an array Is it better to keep
these together?

### todo

- ~~keep garbage collector around, just refresh~~
- test performance
- two stack repository for functions
- memory pooling for array of upvalues & maps of closures and functions

Note: only a few allocation strategies make sense. Varied size, but nested
lifetimes? Use a stack. Fixed sized but random lifetimes? Use a memory pool. The
rest requires more creativity.

The two stack solution for functions presupposes that functions are never freed,
something that is possible now. Maybe that is an optimization for rlox, as it
give the garbage collector less to manage. I think I mostly need a break from
this project, though, maybe try these ideas on my other projects.

## 2024-09-04

### nanboxing

The verdict is in:

- binary_trees: failures
- equality: loop 5.19995379447937 elapsed 5.391582727432251 equals
  0.19162893295288086
- fib: 2.4675538539886475
- instantiation: 0.7299289703369141
- invocation: 0.6673479080200195
- method_call: 0.46359729766845703
- properties: 0.9386715888977051
- string_equality: loop 1.5483124256134033 elapsed 1.6908936500549316 equals
  0.14258122444152832
- trees: failures
- zoo_batch: 134940000 2249 10.003165483474731
- zoo: 10000002 0.6845400333404541

It helped. The difference between equality and string equality is rather
notable. In one test the data is on the stack, while in the other it is on the
heap as constants of functions, so that may explain it. I guess it has to do
with constant lookup, since that is the apparent difference. the reason we
didn't put all constants into a master list, is that functions can be nested, so
constants would also show up that way.

The tree and binary tree test still fail rather mysteriously.

okay, not so mysterious, apparently the initialized is forgotten during the mark
operation, after which

### restoring the bytecode idea

There could simply be two stacks: One for functions that are in progress and one
for functions that are done. once a nested function is closed, its code,
constants and lines are moved to the done stack, and the compilation of the
parent functions continues. The end result is that on both sides only one array
of code, constants, lines and run lengths is needed. The compression of lines
could even wait until that point. Also note that the order of the code could be
reversed, so the ip would decrement every time.

### todo

- keep garbage collector around, just refresh
- test performance
- ~~NaN Boxing~~
- two stack repository for functions
- memory pooling for array of upvalues & maps of closures and functions

## 2024-09-02

### one array of upvalue handles

Idea: have one vec of upvalue handles in Closures, and gice each closure an
offset into this array. Problem: how to handle free space there?

Initially, this seemed like a simple matter of moving all the upvalues together
to the start of the vec, and then updating offsets. After doing that once,
however, the freed handles get space at the end of the list, and then... there
is a risk that not enough space was allocated to move the out of order offsets
into. And it is an issue that we need the free lengths.

General advice is to sort by size, Which is entirely possible. Technically, each
upvalue count could be treated as a separate type with a seperate pool.

What about this: Allocate space in batches of 256 Attach a subdivision to each
batch, like 'each element should take up 5 slots.' Towards the larger sizes,
this is as bad as buddy, but consider this: out of 256 only 1 slot is wasted,
and 51 elements are stored while buddy would roun dup to 8 and only fit 32
element because of it.

Every time a new allocation is needed, it goes out of the last available batch
for that size, until full. It is easy to move objects in and between chunks of
the same size, So all that is left is to keep track of those sizes? Some the
chunks may not be full and

I imagine that this batchtes comon interleaved out of a larger memory pool, but
the interleaving could be an issue.

1. Each chunk could know how much space is left, and where to look for more
   space of the same size
2. The allocator has for each size, the first chunk with space available

What handles does this give? It would just be the 'broken' system, where the
first part of the handle points to a chunk, and the second part to of offset
into the chunk.

{ item_size next_ptr free_ptr ... } How big can allocations get? Note, for
really big objects, don't use this stuff. This memory pooling is just for tiny
objects. The size of the chunk is a fair upperbound, but it schoukd probably be
less The item_size requires one byte, the next prt could be usize the free port
point within the chunk, so one byte is sufficient again. Maybe these details
should be stored externally...

I don't see this solution moving around objects yet. Perhaps that is simply
correct: the system uses a free list, differentiated by size. The handles don't
change

### testing

Cannot run the all benchmarks because of some bugs.

- binary_trees: failures
- equality: loop 5.858097553253174 elapsed 6.3113532066345215 equals
  0.45325565338134766
- fib: 2.7665443420410156
- instantiation: 0.8608591556549072
- invocations: 0.8606352806091309
- method_call: 0.5624816417694092
- properties: 1.2129974365234375
- string_equality:loop 2.1379945278167725 elapsed 2.297173500061035 equals
  0.1591789722442627
- trees: failures
- zoo_batch: 113580000 1893 10.005820751190186
- zoo: 10000002 0.8029170036315918

for comparison:

- binary_trees: 6.599929571151733
- equality: loop 3.3784124851226807 elapsed 3.054243803024292
- fib: 1.9488043785095215
- instantiation: 1.4910368919372559
- invocation: 0.614128828048706
- method_call: 0.9227621555328369
- properties: 1.6836957931518555
- trees: 12.388887405395508
- zoo_batch: 1324
- zoo: 1.33463716506958

So fib is slower and tree tests fail for some reason. Equality tests are slow as
well. Much appears to be faster, without me trying very hard, though.

## 2024-09-01

### binary heap search

What happen if you start from the end? Can a heap have duplicate values? Perhaps
this requires a traversal, Than can be use to insert more efficiently afterward.

### new garbage collector

New garbage collection causes duplicated code. No solution for this yet... Maybe
use the KINDs and arrays instead of separate fields.

### data oriented rlox: transform complete

All unsafe code was booted from the project, since everyhwere memory pools are
used implemented on top of vec. That is a big win, but I doubt this is fast. But
maybe first look to clean up code.

Looking forward:

- ~~put functions in heap~~
- less code duplication
- 'maps' instead of 'map'
- buddy allocator for arrays and maps
- ~~other solutions for storing and finding open upvalues~~
- keep garbage collector around, just refresh
- test performañce
- NaN Boxing
- closures as arrays of u32
- ~~get rid of linked list in compiler.~~

### allocation strategies

For the maps the buddy allocation looks like a great fit, since the underlying
arrays how to have power of 2 lengths.

Closures have arrays of upvalues which can have varied sizes. How do you
allocate those? o/c the garbage collection provides a bit array, so after
collection, The free list could contain runs of free slots, and Upvalues could
try to find the tightest fits for new closures.

It is easy to overlook something here. The closures cannot all have upvalues
adjacent in the memory pool, since they share upvalue with eachother. Arrays of
handles are needed in this case, not arrays of values, which is what my plan
might do.

A memory pool that provides arrays of arbitrary size, of any other type of
value, could use the bitarrays to find free runs of values for the free list.
When an array of certain length is requested, the pool searches the free list
for a good fit. If more space has to be allocated, then the entire range becomes
a big free run.

Dual: if closures are done this way, allocate some extra space for the funcction
handle, and hand out a handle that works for such an offset.

The buddy allocator can allocator arbitrary sizes, just put the remainder on the
free list again... Extra bookkeeping needed for freeing correctly...

## 2024-08-31

### new garbage collections

For collectibles:

1. have a list of used handles
2. have a bitarray of marked handles
3. have methods for tracing and sweeping
4. loop through the pools: trace and mark their elements,
5. loop through the pools again, to sweep them.

To centralize, put all the pools on the heap.

Every time we trace a pool, new handles if many types may be found.

Two big changes: data orient upvalues and refactor tracing completed without
break any test.

### sorting open upvalues

The reason a linked list was used before, was to keep upvalues stored by stack
location. When closing upvalue, no vull traversal was needed that way. I
experimented with keeping a list of open upvalues equal in size to the stack,
This seemed sadly slow. To close upvalues we need upvalue handles associated
with each stack location pointed to.

What the current thing is doing is different because it can create duplicate
upvalues.

We need a bijection between stack slots and upvalues. Form upvalues to stack
slots is sorted. From stack slots to upvalues, I don't know yet. Genuine
application of heaps... or put the next pointer in the object like Munificent
did. Maybe us a trie to safe space... or implement linked list atop vecs. I
mean, in a data oriented way: keep a vec of next highest open next to the open
list, use it to traverse the open ones.

Something clever: stack ref uses additonal _stack pointer_ to point to the next
upvalue on the stack. Problem: upvalues don't go on the stack.

I'm really opposed to the linked list solution.

### status

Setting on a solution with two stacks, and moving values between them. What are
the requirements?

It is scope based. upvalue closing happens when upvalues go out of scope. so
sometimes a single upvalues is closed, and sometimes the collection for a
function. The heaps structure is valid.

Binary heap push on top and rebalance. Opposite: pop from end, make root, and
rebalance. can use peek and deleteMax instead of top. But get?

## 2024-08-30

### type specific allocators

So have a struct that owns all the allocations of a type. It seems like we
should just follow the example of vec, but with a solution for free elements,
like a free list.

Note: we need a different approach for dynamically sized types--array in
partcular--and a way to keep track of free elements.

### after implementing

I have the memory pool worked out, but I don't veel like using it yet... I feel
like the data oreintation may reveal some interesting alternatives.

- I think it should essentially be a vec, with the mechanism for generation
  handles separated.
- Is it really that great, to used the memory in the pool for a links list of
  free slots it safes memory.

### the linked list of open upvalues

Is that really that performant? The amount of time it takes to insert an upvalue
is linear just push one and sort, or even go through the whole array onse when
clsing is also linear. Hmm...

## 2024-08-29

### serious issue with bytecode

Nested functions... I see the code of nested functions nested in the byte code.
Make sense with how the compile works, but then code is executed immediately...
the return statement would just take you out. The weird values are apparently
explained by a function running when it shouldn't.

Some of these things could probably be fixed with jumps. Just jump over the code
of other functions. Here we finally see what the chunks are for, however: each
function's code is in its own slice of memory, and therefore adjacent in memory,
unhindered by nested function definitions.

Time to admit defeat... Munificent was right about both the data model and the
need for a good dissambler.

### result

Reverted the data structure solved the recursive function. something still goes
wrong with upvalues, and new problem have arisen with classes.

Can a mistake ever be smart? At least the tests succeed again, after severla
significant refactors.

### type specific allocators

So the idea was that for each type, there is a class that manages objects of
that type.

## 2024-08-27

### type specific allocators

Just do it like that: have a memory manager with a type parameter, for managing
allocations and pointers of the type, perhaps with the full garbage collection
package, support for arrays perhaps (support only arrays?).

### status

Introduced bytecode object they contains all code and functions. Two failing
tests left, and no good error message.

## 2024-08-26

### the obstacle

To get closer to the original, rlox's heaps should provide empty objects, to be
initialized afterward. But this initialization after the fact can now change the
size of the object, as a lot of reallocation is going on behind the scenes.

Let's copy the zoo here again:

- BoundMethod: instance: Instance, method: Closure ~ fixed size
- Class: name: String, methods: Table<Closure> ~ dynamic size
- Closure: function: Function, upvalues: Vec<Upvalue> ~ dynamic size
- Function: name: String, arity: int, upvalue_count: number, chunk: Chunk
  ~apparently dynamic size
- Instance: class: Class, properties: Table<Value> ~ dynamic size
- Native: function_pointer: usize
- String: hash: int64, content: Box<str> ~ dynamic size
- Open Upvalue: stack pointer + pointer to next open upvalue (because a linked
  list is used here)
- Closed Upvalue: value

- Value: tag: Nil, True, False, Number, Object... value: f64|usize
- Chunk: code: Vec<u8>, lines: Vec<u16>, constants: Vec<Value>

### broken handle tactic

Assume the pointer is an offset to a base pointer held by the heap. Now
actually, the heap uses a sequence of pointers each pointing to a page of memory
of some pwo of 2 size. The trick: use the top bits to decide which base pointer
and the bottom bits for the position within the list. Attempts could be made to
use larger chunks of memory when reallocating.

### chunks and functions

Instead of having chunks, there'd just be a big chunks repository, that contains
the data of all chunks put together, but that creates new opportunities. e.g.
put all the code in one big array, the constants in another, and the lines in a
different structure altogether. The functions have offsets into these tables,
and work with that.

The names of functions don't matter, until they are printed. So this could be a
slower operation hat goes back to the source and parser the function name again.
That might not be the best idea.

We could do the run length encoding for the line numbers. To start with. Only
the offsets into the code and constant arrays actually matter, So change the
functions to have only those. And now the functions themselves make more sense,
as they contain only offsets into the global store of chunks and constants.

Yes, this is the next place to go.

### eventual allocation

I can imaging this working out as follows: the compiler at first pessimistically
allocates a lot of space for code & constants. Then realloc is uses to downsize
the array, and free the space for other stuff. If too little space was
allocated, the broken offset idea could still work.

### the new repo

Is actually a runtime representation of a compiled lox file.

## 2024-08-25

### big migration

Suprisingly few failing tests after such a big migration.

For strings, reference counting may be a superior strategy, since curcular
references cannot happen. Trouble: simply dropping string handles must be
stopped: this can only be done by handing the pointer back over to the the
stringpool.

Reminder `$env:RUST_BACKTRACE="full"; cargo run -- test.lox`

### now what?

- try building an allocator, i.e. something to actually keep track of allocated
  space, including what is needed for strings, tables, chunks etc.
- more subdivisions: seperate pools for every type of data.
- optimisation/simplification: so many steps are needed for a function call,
  maybe that can be improved
- upvalue changes: shouldn't open/closed upvalues simply be differed types of
  value or kinds of handle?
- value change: use value arrays that store tags and content separately instead.
- static/dynamic split: separate memory managers for such data.
- call stack changes: don't use the handles to the closures, but copy their
  content straight into the call stack. Just doing so for the top closure could
  help.
- numbers: change something fundamental, so the 64 bits of each number are no so
  big and bad anymore.

### upvalues

Upvalues were pointers. The open ones point to values on the stack. The closed
ones point to values that are moved the heap. To support closures. The move to
the heap should happen without updating the closures, so once again, the upvalue
hanlde should not change when the upvalue is closed. Otherwise...

I think we tried this, keep a vector parallel to the operand stack so the open
upvalues could be stored next to them.

### closures chunks and functions

Looking for optimalisations of the call stack, I wonder: Is a chunk ever really
independent from a function?

Note that we now effectively use handles for instruction pointers. I.e. there
could be an instruction store, that has everthing in it, code, lines, constants.
Offsets would be needed for each. functions are not that different as it stands.

### numbers

If numbers are somewhat exceptional, then more indirection might work: store all
numbers on the heap, and use handles for them as well. separate storages for
numbers might also do some good. A special arithmetic stack or register. o/c the
vm is gaining multiple repositories that still life in the same underlying
memory.

### allocators

Clox needs to manage all of its memory without the help of rust's borrow
checker, In particular the vecs and the boxes I am using would be on the heap in
those systems. So what about changing that up?

Consider what types en sizes of vec or box<[...]> are used, allocate a big one
for each, use offsets and lengths as handles. Note: powers of two means the
length could perhaps recorded in one byte. Also, the stores could keep track of
this data, instead of the handles, if that makes things easier.

What is going on with the allocator in `RawVec`? Allocator is a trait. It has an
instance called Global, which is a zero size type, hence the acces to global.
This is how `Vec` can access an allocator without having to pass a pointer to it
around.

### multiple heaps

Advantage: by having each control its own type of element, the need for unsafe
casts disappears. Tracing would be more complicated: each type would need it own
set of marks.

Note: technically each type has fixed size. Sure, an object has an array of
string values pairs, but those are slices of other heaps... I should think more
about this: arrays of any type, but this require adjacent handles.

Currently data is created on the stack, then moved to the heap. This is not how
clox does it, but a simpler solution for rust. The benefit of the current
solution is proper initialization. Also, byte counting is easier when the
objects contain variable size elements.

### serious bug

Compute has of 'a', find its position in the string pool taken, but find a space
for it 2 positions down. Grow the keyset, now the space of 'a' is free, so 'a'
receives a second handle, which fails all equaility tests. Of course, this only
happens with tiny string pools, but I happened to test this out. This is why we
cannot bump the hash code.

Genuine hash collissions vs accidents like these: how do we solve this?
Generally handles should be hashcodes. If and only if another string already has
the hashcode, move one up. Could this still fail? Yes, because of garbage
collection: that could remove the other string with the same hash

So, use generations?

## 2024-08-24

### the big push?

The list of handles is there anyway. We now need to add a header to each object
that has the kind (for proper garbage collection) and a marker boolean (to mark
which objects should be kept.) The difference is that the

How does mark and sweep work now?

- are put on a 'todo' collection
- one element is taken from the collection, if already marked, stop if not, mark
  it and put dependencies on todo
- todo list empty -> start sweeping. To make marking quick and easy, the marks
  are in the object headers.

Of course, object headers could be stored in a seperate list, as soon as handles
are used. Changing handles seems expensive. This is done currently to get all
free slots at the end of the vector of handles. Would it be so bad to just keep
a list of free slots?

How do we get that list after a sweep? The marks are next to each object. Going
through the list and storing the free slots seems like the only way.

Count how many object are marked to decide whether to grow the number of slots.
Use linear search for free slots.

Tombstone kind at sweep. Marked set only has to exist durign mark and sweep.

### requirements

So the idea now is to not do the by class thing now, but to replace the heap
with one that returns handles.

1. the mapping from handles to pointers should be fast
2. finding free handles may take some time, but should not rise linearly with
   the amount of memory used

It seems obvious that there must be a vector of pointers and that part of the
handle is an index into these. It could be broken up: first half of hanlde is an
index into the vector,

The free list could just have slices of free memory. Subdivide according to
size: no matter how much space is asked, always bump up to the next power of
two, then assign a block of that size.

We could do something with a few large slots and handles that can peek inside. A
free list would be nice for fast allocation. Free list for elments of differen
sizes? So the idea would be to allow slices of memory powers of two size.

### what do we need variable allocation sizes for?

Mainly values, upvalues, closures, code, strings and line numbers. For the
values, the only having power two size options is not limiting, Since that is
what is used in the table anyway. I.e. we could have a free list with slices,
and break the slices in two until it fits the requested size. The question then
becomes if two half of a slice are free, Do we every find out, or will memory
remain fragmented? Does it matter much if memeory does remain fragmented?

What I am planning here is too complicated.

- just do the list of pointer + list of free indices thing first.
- free slices might be a good way to make the list of of free incides smaller.

### the tangle

There is now a triangle relation between the heap, the string pool and all table
shaped objects. Part of the problem is that tables can resize, and the heaps
needs to know that this has happened because the number of allcated bytes
determines when it is time to collect the garbage.

- use the new strings, this cuts heap dependencies.
- refactor tables, to be more specialized, and embrace their dependency on hte
  heap
- forget about making garbage collection too nice for now: just come up with a
  different criterion for running the collector than a guess about allocated
  bites.

## 2024-08-23

### more data orienting

Ideas:

- replace `Local` with `Locals` and `Upvalue` with `Upvalues`
- same treament for `CompileData` into a collection

Currently the chunks store line numbers. Computing these number requires the
source string. This is an issue because I want to move the code out. Suppose we
store the token offsets instead... Can we compress them? Sure, store the
differences! Can we limit the range of differences? Hard to do because of
whitespace, strings, etc. Have a table for them.

Messing with locations, how nice is it, doesn't realy help with anything.

- emit functions cannot move, since we still need the source
- the vm does not have the sources.

## 2024-08-22

### more data orienting

First store identifier names in the heap, then use the handles for computations.
Up to now, I've always had problems with lifetimes and such. This is gone now.
Some easy things to change in the compiler: collections of locals and upvalues.

## 2024-08-21

### tokens

Munificent reduces the tokenizer ot an iterator, so none of the token are
stored. Andrew Kelley now reduced token to a 'tag' and an offset into the
source, as other properties can be computed. Of course, not storing a list of
the tokens reduces the space benefits form making them smaller, but it would
help against the lifetimes, and where tokens are stored in the compiler, the
tags aren't always necessary.

Consider:

- error token: just run the scanner from offset to see what the problem is
- line and column: go to the scanner, scan for newlines from the beginning. Much
  easier, and since this is mainly needed for error messages, why not take some
  extra time? Put offsets instead of lines into the chunks as well: how much
  help are specific numbers if you cannot access the source code anyway?
- lexeme: if you know the type, just run the tokenizer from offset.
- Kelly notes that 4GB is a reasonable limitation on source files, so the
  offsets can be u32, taking up as much space as line and column numbers now,
  but con

In any case, this seems simpler and easier to test than the new string stuff.

## 2024-08-20

### generations and handles

Idea: always create unique handles When growing storage space, merge generations
together. How does this work? After filling the last slot, a new table is
created that is twice as big The handle determines the position in the new
table, with the even generation ending up in the lower half. Generation
automatically split in two.

There are some functionalities:

- new entry
- mark
- sweep
- grow

I need to work out the new mark-and-sweep mechanism: when is is triggered? how
are the marks stored? how are the sweeps done? when is space actually allocated
and freed?

### strings

Focus on this now, and keep close to the original where you can. perhaps doing
the mark and sweep by rebuilding the keyset of all strings is a good approach.

### fighting the compiler again

Munificents linked list of compliers is not cooperating. This was part of the
reason data orietations intersted me: the talk was about imporivng the zig
compiler by following these principles. And the ideas seem Rust friendly.
Instead of appeasing the compiler somehpw, why not start the refactor? Either
way, I am a long way from testing anything.

Yep, compiler warning are gone. The trick seems to be to avoid references if not
self args.

Well at least I could test & debug the scanner again.

## 2024-08-19

### Trying strings

Whatever service controle string allocations, will need to be passed around a
lot. Rust will add a lot of pain to this.

Loxtrs should have:

- hashes
- the string content
- the copy and hash stuff
- a way to sweep for garbage collection, likely including some collection of
  either free or taken slots. o/c
- a way to prevent allocating the same string twice, based on hash code.
- a way to grow if more strings are added than can currently fit.

Note that a string is a collection, so slices, as already popular in Rust, could
be used: Just allocate a big array if byte, and copy each string into it as
needed. The _fat_ handle would hold the starting point and length.

Loxtrs should produce a stable index for each string, so hash code is a good
starting point. But there could be collissions, so add a counter just in case.
Why not just use the counter?

It could be a hash table like design, where one first searches the unstable
current index for the stable hash code.

Advantages of integrating the hash: no need to compute it everywhere the string
is used as key or comparison. Since Loxtrs would be a hashset of strings,
finding collidings strings should be fast (shouldn't it?) Disadvantages:
actually accessing string content is indirect. Anything from printing to adding
strings together would involve extra steps.

The empty slots in the table are the points at which searching for matches
stops. That makes it strangely essential that some space remains unused.

Now about the second part: when two strings actually have the same hash code,
some extra counter has to go up and tell the difference, right? Is this counter
affected by entries being sweeped? I guess the same string could just returns
with another handle. The main concerns is that the counter runs out, if the same
string is created and destroyed too many times. opportunity to use a huge
counter, I guess, and not to sweep too often.

Note that we have options to

1. never evict
2. only evict right before growing the string pool. I.e. mark strings as unused,
   but only actually clean up when running out of space. That way it may be
   possible to resurrect strings that normally would be garbage collected.

Not the indenpendence here: the service receive information about sweeps, but
uses that information only when it needs to. Should it trigger sweeps?

Reference counting may be an alternative: this would require calling Loxtrs when
handles get dropped, and I don't see Rust collaborating there.

### how about

Doing the simpler case of native functions first? Such an improvement already...

### one more thing

1. Build the keyset construct--which is a collection of string handles
2. Build all tables as keys sets with arrays of values attached
3. Let the string pool be a privileged table with &str in it, and maybe some
   other stuff.

Hashes are still pretty long, so we may not be saving much spaces this way.
Wait...

If a collision occurs, why not just take hash + 1? Just think about what the
index would be if we could just have a table with 2^32 rows: the second string
with the same hash code would receive hash+1 as index. Back to the keysets: they
fold this into a smaller range to safe space, and may have to shift the keys
again. This is fine.

## 2024-08-18

Some fresh ideas:

- https://www.youtube.com/watch?v=IroPQ150F6c
- https://floooh.github.io/2018/06/17/handles-vs-pointers.html#:~:text=Handles%20are%20the%20better%20pointers%201%20Move%20memory,Some%20real-world%20examples%20...%207%20Update%2028-Nov-2018%20

Instead of allocating many small objects, allocate an array for each field of
each class. Indices are used as pointers, so it is important that by class the
fields of each object are stored at the same index.

Interesting to consider how memory could be managed by class: garbage collection
happens for a speciifc class when its field tables run out of memory, instead of
for all objects at once. Moving object to a new table is natural, changing the
index is not, but perhaps 'freed memory' could just be stored in a stack by
class, and be used for new allocations. Reference counting might be easier as
well...

This inspires a language feature to indicate an allocation limit per class: a
singleton does not need a table, and if only a small number of allocations are
expected, the system could produce smaller pointers for such objects and smaller
tables.

Hashes instead of indices? Hashes are only nice for immutable objects! Even
then, the indices would change if the tables had to grow in size.

Of course, arrays as data types become a different matter: and array valued
field would still contain actual pointers. I suppose the fat pointer idea
applies here: the field would actually be stored as two fields, one for
array_size and another for array_content. Similarly, generic fields could be
broken down into a class and an instance field, and tagged unions into a tag and
a data field. Dependent type style implementation.

If this was a good idea, then has nobody tried it yet? Perhaps functional
programming is better served this way.

What about the stacks? In particular the call stack can be broken up into
seperate arrays of instruction and stack pointers.

Rlox values are tagged unions! So have two operand stacks, one for the tags and
one for the data. Perhaps give another shot to using tags for different kinds of
object in rlox: the data array could be a unions of different types, and be 8
bytes, while the tags are 1 byte, and sometimes contain more relevant data in
that one byte.

Note: the compiler now allocates seperate chunks of code. It could also be just
one big array of 'instructions'... that would make instruction pointers longer,
though.

The string pool feels like a justification for just putting every type into its
own pool.

I get the feeling this may be a way out of much unsafe code, and the tangle of
dependencies inherited from clox.

### details

To get an indication of the tables that need to be build:

- BoundMethod: instance: Instance, method: Closure
- Class: name: String, methods: Table<Closure>
- Closure: function: Function, upvalues: Vec<Upvalue>
- Function: name: String, arity: int, upvalue_count: number, chunk: Chunk
- Instance: class: Class, properties: Table<Value>
- Native: function_pointer: usize
- String: hash: int64, content: Box<str>
- Open Upvalue: stack pointer + pointer to next open upvalue (because a linked
  list is used here)
- Closed Upvalue: value

- Value: tag: Nil, True, False, Number, Object... value: f64|usize
- Chunk: code: Vec<u8>, lines: Vec<u16>, constants: Vec<Value>

Note: constants can be numbers, strings, maybe functions, but not every type of
value.

Upvalues are tricky. Perhaps a different set up could lead to a simpler
solution. using a bit to decide between stack and heap pointer might work.

So one set of tags for values, with all cases getting their own stack pointer
variant, a only one separate tag for open upvalues, as the stack pointer tells
the tag of the value there.

One more thing. Apply this to collections, and the result get weird. A table for
the 5th element would have many empty slots. That is not the way.

### garbage collection

Note: clox has all memory would be managed explicitly, and builds a system that
does that for all memory including compile time objects. Rlox uses both the
compile time management, and the stuff present in collections. The cycle
ultimately works like this:

- the vm asks needs_gc, which is based on a rising threshold for allocated bytes
- if yes, a mark and sweep is done to free up memory.

The data oriented system would have preallocated space for each object, so a
quick response with any unused row should be possible. When no unused rows are
left a bulk optimisation is done. When does actual garbage collection happen
then?

If any of the class tables is full, the first step may still be a mark & sweep,
that results in a list of free rows for each class. Afterwards, new allocations
go into these free rows: the indices can be stored in a 'freed' list, that is
exhausted before using new rows. Only if mark and sweep does not help here, then
a new table is allocated and all objects are moved. Is that the way?

Note how mark and sweep needs to go through all memory anyway, even if only one
type of object is full. And it can fail to free memory, at which point a huge
chunk of memory needs to be moved.

Perhaps it is beter to change the indices: to mark objects, copy them into new
tables, while keeping track of the changes in indices.

Stick closer to the original: have an extra column to tell which rows are empty.
Then when space is needed, use this column to find it. There could be a cursor,
to keep track of promising places to look for space, or maybe a smart search
function (bloom filter?), that doesn't have to check each indivual row to find
an empty one.

Scattering data may decrease searches for free space, but increase cache misses.
I have no idea how to test either.

Note the new issue, which is that we need to move an entire table on some
cycles, reallocating all object of a class. Instead,

Instead of doing by object allocation, the system does a big allocation upfront
and again when capacity runs out. Of course, it could still be incremental: the
top level could just be a list of tables that maybe double in size. Allocating
extra space just means allocating an extra table. No objects actually move. The
resolution used a two part pointer. The size of the tables could also be the
same, carefully adjusted to the needs of the cache.

### special array support

If ever needed, the object tables support arrays-as-slices again: asumming that
and array of objects can always be allocated to have adjacent indices in this
new style heap, an array just needs a class, a length and a start index.

This an idea about a new kind of object: just have a table of properties, and
point to the start of the object in that table. It won't work if objects can
gain new members, of course. It might work for classes in rlox, though, since
the list of methods is fixed.

### key set class

Perhaps the table should be split into a keyset and an array of values. The
keyset serves as string pool. The array of values is kind of a side show in most
cases, and perhaps could benefit from specialization. Like the `table<value>`
could eventually be a `(keyset, tags, union { pointer, f64, ... })`.

Lox strings are special anyway, so maybe build their memory manager first.

## 2023-06-11

### globals and repls

Globals could simply be kept on the stack, and subsequent scripts compiled to
rever to those entries. So the stack is not entirely cleaned between repl
entries, and accessing globals gives a compile time error, instead of a run time
error. Dry solutions.

## 2023-06-09

- one stack of locals
- & mut instead of Box for compilers

### thoughs on recursive descent

The attraction is that the grammar froms a rought outline for the functions that
make up the parser. Each nonterminal a function, calling other functions in
branches and sequences as the grammer dictates. This is good enough for
producing the abstract syntax tree, but a single pass compiler needs to peek
down the stack for information, in particular which variables are in scope.

Options:

- reify the stack: store data on the heap that represents the context
- variable passing: pass the information allong with every function call

The latter can be simplified with object oriented syntax. The objects can
contain objects lower on the stack, but not the other way around. This means the
links is the wrong way around now.

### result

Made it work with unsafe pointers. O well.

## 2023-06-08

- ~~add README~~
- ~~add to munificents list~~
- ~~reddit?~~
- one stack of locals
- ~~& mut instead of Box for compilers~~

### gc thoughts

I don't like how the heap is dependent on other to supply the roots, but what
options do we have?

- move all run time memory into one struct and put the logic there. Note: this
  is all of the VM, actually
- somehow keep track of roots, like assume new elements are rooted, then signal
  when throwing them out. Sounds like overhead.

Another point: because the heap is no linked list, it will occassion have to
resize, added extra garbage pauses, when it doesn't collect anything. Maybe the
trigger should be a combination: either reach a number of objects or a number of
bytes, and then use the sweep in place or on resize.

### ending this project

I think I am done. Learned a lot to use in another project.

## 2023-06-07

- add README
- add to munificents list
- reddit?
- one stack of locals
- & mut instead of Box for compilers

### adding table

Big performance gain!

- binary_trees: 6.599929571151733
- equality: loop 3.3784124851226807 elapsed 3.054243803024292
- fib: 1.9488043785095215
- instantiation: 1.4910368919372559
- invocation: 0.614128828048706
- method_call: 0.9227621555328369
- properties: 1.6836957931518555
- trees: 12.388887405395508
- zoo_batch: 1324
- zoo: 1.33463716506958

We're in no clear winner territory.

### miri

Tried: `cargo +nightly miri test 2> error.log` Found hundreds of leaks. Possible
reasons:

- miri is not smart enough to understand the garbage collector.
- there is a memory leak, because we are not freeing memory as we should.

I am using boxes for allowcation and deallocation, but strange things are
happening. Everything seem already until drop: at that point the memory is
already filled with strange data. Why though?

Hunch: Kind and bool are small, so their layout is heavily optimized. Whatever
it was, turning handle into `*mut Obj<u8>` seems ot have fixed it. The only
error remaining is that mrir cannot handle `clock`. I actually debugged a memory
leak with miri!

### new problem

The byte count check was calling free instead on the byte_count method! Removed
it, because it might hurt performance.

- binary_trees: 6.376673221588135 (better)
- equality: loop 3.703075647354126 elapsed 3.112445116043091 (worse)
- fib: 1.9507935047149658 (worse)
- instantiation: 1.9068045616149902 (worse)
- invocation: 0.6107420921325684 (better)
- method_call: 0.8883857727050781 (better)
- properties: 1.6284961700439453 (better)
- trees: 12.144921064376831 (better)
- zoo_batch: 1248 (worse)
- zoo: 1.1798393726348877 (better)

The differences are small, though, so it might be background processes.

## 2023-06-05

### benchmarks

When running benchmarks, use `--release`, otherwise the run will be slow.

- binary_trees: clox: 6.474 rlox: 14.052
- equality: clox: loop 6.343 elapsed 8.136 rlox: loop 6.635 elapsed 6.676
- fib: clox: 3.978 rlox: 5.064148664474487
- instantiation: clox: 2.993 rlox: 3.9380042552948
- invocation: clox: 0.956 rlox: 4.011815547943115
- method_call: clox: 0.806 rlox: 2.945194721221924
- properties: clox: 1.638 rlox: 7.1311938762664795
- trees: clox: 11.644 rlox: 32.88513708114624
- zoo_batch: (number of batches processed) clox: 1102 rlox: 316
- zoo: clox: 1.401 rlox: 5.305315971374512

Property access is expensive as expected. So let's fix it?

### another attempt at Loxtr

Basically make it a `str` with a pre computed hash code.

- binary_trees: 8.950028419494629
- equality: loop 3.950610399246216 elapsed 3.556746482849121
- fib: 2.7511284351348877
- instantiation: 2.541790246963501
- invocation: 1.1747519969940186
- method_call: 1.3273963928222656
- properties: 2.8201920986175537
- trees: 16.289510488510132
- zoo_batch: 760
- zoo: 2.0269880294799805

I am pretty happy with this improvement. Rlox is somehow beating clox on some of
the benchmarx how? Because the garbage collector is less busy? Is it the
constant table optimisation? Property access has much improved.

Right, vs code didn't know it was supposed to keep the benchmarks on separate
lines. O well.

### so what next?

Compiler performance maybe.

The upvalues are a linked list, because of the desired behavior:

- inserts can happen everywhere, but are more likely at the start,
- upvalues are removed from the end. So an alternative is a stacksize array of
  `Option<Obj<Upvalue>>`, It takes up more space, but inserts require constant
  time, and closing depends on how many values get allocated with a frame (max
  256...)

I doubt the difference will be notable, and it may be negative, but this
solution is simpler. Okay, it is slower! Like 20% on a few of the benchmarks.

### tables

What I am use to for arrays is actually `Box<[T]>`. This could be the basis for
a `Table` implementation.

Porting table wasn't hard. Let's see what it does for performance later this
week.

## 2023-06-04

- ~~using a custom allocation to count allocated bytes~~
- add README
- add to munificents list
- one stack of locals
- ~~replace Kind with fat pointer to trait object: the 'handler' idea.~~
- & mut instead of Box for compilers

### byte counting allocator

Already found that replacing the allocator requires getting into unstalble rust
apis, and I don't want to do that now. To make matters worse, allocators are
supposed to be immutable, so another workaround is needed for counting.

Alternatives: guestimate sizes on allocation... Analysis:

- strings are heap allocated, but immutable in lox
- chunks have heap allocated members, but don't change size at runtime.
- classes have heap allocated members, but these costs are determined at
  initialisation time.
- this leaves instances, which are the only actually dynamically sized
  structures.

If the compiler could resolve super classes, then method tables could be
constructed by the compiler. Did a little test:super classes can be dynamic, so
the size of a class is not determined at compile time.

This is a plan: when storing object on the heap, estimate the number of bytes
allocated do extra updates when building classes and setting properties.

### Traceable vs Tracer

Maybe separate the trait that connects the kind to a type form...

reason not to work with `dyn` is that fat pointers take up more space. but
couldn't 'kind' be replaced by one fat pointer?

Can you do `type Target` and still be object safe? All because a fat pointer
takes up too much space.

I am not content with the visitor now, but maybe it can work with a macro
instead.

### less clumsy call

What if we waited to get the arity from the stack until we need it?

## 2023-06-03

- add README
- ~~more clox like string implementation~~
- using a custom allocation to count allocated bytes
- & mut instead of Box for compilers
- one stack of locals

### lifetime stack

For the compilers: a stack of &mut where each Compiler lives somewhere on the
stack. The deeper in the stack, the longer the lifetime. I cannot explain this
to rustc yet, if it is possible. The lifetime bounds only allow 'outlive'.

### LoxStrings

To do the hashset thing, Eq and Hash are needed on `Obj<Loxtr>`. I was think
Rlox should just allocate everything in one go, inclusing the space for the
charaters of the string, but Clox doesn't function that way.

HashSet with normal Strings does the job: storing the hashcodes with the strings
is not necessary.

### constants

Each time a script access a global variable, a constant is added to the chunk,
and there is no dead code analysis. The string_equals benchmark therefore fails.

## 2023-06-02

Everything builds now. Finally.

### ideas

X visitors to improve on handling differrent kinds of object

- more clox like string implementation
- using a custom allocation to count allocated bytes X actually test the garbage
  collector

### testing

Downloaded tests form munificent. Could not get through benchmarks, but the rest
seem ok.

## 2023-05-31

### upvalues

Take up an additional byte to distinguish 'locals' and 'non locals'

### 

We could make it more clox like by wrapping uszie in a struct and defining
Deref... How to do the implicit dependency on the stack?

## 2023-05-29

Another thing to note: I don't have to hunt for unused code. Rust tooling just
tells Vscode. I feel like a communist visiting a capitalist supermarket for the
first time.

### frames issue

Copying the top frame into the run method means that the changing instruction
pointer is not stored. Which cannot wokr well. So a reference would be good, but
it confuses the borrow checker.

### jumping around

Why jump a distance, if we know the absolute coordinates of the code? I guess
the relative numbers are just smaller.

This is giving me a lot of trouble now.

### same size instructions

Rlox would probably work with all instruction 24 bits. with 32 bits, it could be
a register machine.

### latest bug

Running `cargo run -- test.lox` when evualating `b = temp + b`, `b` equals
`"outer a"`, a value set to another local variable by another compiler. Why is
this value still on the stack? why is `b` resolved this way? Unit tests won't
reproduce this effect.

Double pop: fighting with the borrow checker combined with copying code...

Next one: weird things with function calls.

## 2023-05-28

Shoulder pain is slowing me down, but I am making much progress now.

### rooted elements

Just a thought: keep track of which objects are rooted and which aren't using
the header. Risky? Maybe: callers have to carefully root and unroot elements.
Perhaps the borrow checker can help there... Why this might be a bad idea: if it
is in the header, the garbage collector has traverse all objects twice, at least
once for mark and once for sweep. If rooting and unrooting is done by collecting
objects, however, unrooting becomes more expensive.

### NaN boxing again

Assuming 48 bits pointers, we could have 7 different pointer types. In
particular differences between compile time constants run time variables still
seems valid.

### the necessity for the string pool.

It is important for efficiently comparing names of properties and variables.
This is what clox is build around. Not all strings have to be interned,
although... clox bahaves differenly if these internments don't happen.

I am adding a simple implementation to the heap.

### binary_op macro

I don't see a way to pass operators, but maybe all required operators exist as
methods. Nope, doesn't work.

Rust simply does what I'd've always wanted Scala and Java to do: every method is
an extension method. Doing so also show the limits of that approach: extensions
methods are not dymanically dispatched.

### also...

Do some type checking the the compiler, to emit better opcodes? Difficulty:
variables can change type! Seems especially useful to separate string
concatenation from

### sholder pain

I probabaly put the keyboard too far away, to better see the keys I guess,
causing me to reach all day. This is what is hurting me.

## 2023-05-27

### second serious debug

Ran into a seg fault, but fixed a forgotten change in a macro.

### upvalue at runtime

a collation where we can insert anywhere, but remove quickly form the end. so
like a heap.

## 2023-05-26

### first serious debug

Ran into stack overflow and status_heap_corruption, but got out of it.

## 2023-05-25

Main has stack overflow on 'hello world'. Big frames or endless loop?

The rust version takes more time, but that is because I am trying to learn the
language.

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
originals, with .is*<type> and .as*<type> methods. maybe we can do that with a
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
