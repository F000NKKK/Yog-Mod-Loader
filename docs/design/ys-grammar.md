# Yog Script — grammar

Status: design draft, the syntax of `docs/design/ys.md` made exact. `.ys`
and `.yns` share this grammar; the patch forms (§6) are accepted only in
`.yns`.

The syntax is close to C# — not a copy: declarations put the type first,
locals are `let` / `let mut` (Rust's), objects are made with `new` and primary constructors,
statements end with `;`, names follow C#'s style guide. What C# has no
word for (sides, saved and synced state, components and systems, units,
content references, patches) is Yog Script's own.

Notation: EBNF. `a?` optional, `a*` zero or more, `a+` one or more,
`( … )` grouping, `|` choice, `'x'` a literal token, `UPPER` a lexical token.

## 1. Lexical

### 1.1 Layout

- Source is UTF-8. Whitespace separates tokens; newlines and indentation
  mean nothing.
- `;` ends a statement and a member without a block body
  (`i32 X = 0;`, `i32 Area(i32 w, i32 h) => w * h;`); a block (`{ … }`)
  ends its own declaration or statement.
- Comments: `// …` to the end of the line, `/* … */`, `/// …` a doc
  comment for the next declaration (shown by the LSP and IDLE).

### 1.2 Tokens

```ebnf
IDENT      = letter (letter | digit | '_')* ;        (* not a keyword; '@' IDENT uses a keyword as a name *)
INT        = digit (digit | '_')* | '0x' hex+ | '0b' ('0'|'1')+ ;
FLOAT      = digit+ '.' digit+ (('e'|'E') ('+'|'-')? digit+)? ('f' | 'd')? ;
UNIT       = (INT | FLOAT) unit_suffix ;
unit_suffix = 't' | 's' | 'ms' | 'm' | 'h' | 'b' | IDENT ;   (* t ticks, s seconds, b blocks; a mod's own units by name *)
STRING     = '"' (char | '\' escape | '{' expr '}')* '"' ;   (* {expr} interpolates; {{ }} are braces *)
RAW_STRING = '"""' char* '"""' ;                    (* as written, lines included *)
CHAR       = "'" (char | '\' escape) "'" ;
LIFETIME   = "'" IDENT ;                              (* 'a, 'static: no closing quote, unlike a CHAR *)
TEXT       = 't"' key '"' ;                          (* a translatable text by key *)
CONTENT    = '@' kind ':' res_id ;                   (* @item:minecraft:stick, @effect:charge:overcharged *)
TAG        = '#' res_id ;                            (* #minecraft:logs *)
res_id     = (namespace ':')? path ;
kind       = IDENT ;                                 (* the content kinds of yog-ids, open: a mod's own kinds too *)
```

A unit suffix binds without a space (`20t`, `5s`, `3b`); `20 t` is a
number and a name. `CONTENT` and `TAG` are checked at build time against
the registries of every targeted version.

### 1.3 Keywords

Reserved everywhere:

```
using mod class struct record enum interface component system extend unit
public private internal protected static const readonly new this base
abstract sealed virtual override typeof nameof default
void let mut i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 decimal bool char string object dynamic
ref out move typeof nameof default
if else switch foreach for in while do break continue return
true false null is as and or not when
async await try
server client shared saved synced was tune test extern
```

Contextual (keywords only where the grammar expects them, names
elsewhere): `block item entity fluid recipe screen command where has
every requires patch before after inside around call read write constant
replace add uses local proceed result skip first last it value get set
init mc priority`.

### 1.4 Names

C#'s style guide, checked by the compiler (warnings, like C#'s IDE1006):

| What | Case | Examples |
|---|---|---|
| Types, components, systems, content, enums and their members | PascalCase | `Charger`, `Charge`, `Mode.Charging` |
| Methods, properties, fields, events, constants | PascalCase | `Push()`, `Room`, `Energy`, `Use`, `MaxTier` |
| Interfaces | `I` + PascalCase | `IChargeable` |
| Parameters, locals, lambda parameters | camelCase | `amount`, `target` |
| Type parameters | `T` or `T` + PascalCase | `T`, `TItem` |

In a `.yns`, the game's Java members are seen the same way: a getter is a
property (`getArmorValue()` → `ArmorValue`, `isDay()` → `IsDay`), a method
is PascalCase (`hurt()` → `Hurt()`), a field PascalCase (`foodData` →
`FoodData`). The compiler maps them back to the run-time names.

### 1.5 Directives (`.yns` only)

As C#'s preprocessor, first on their lines: `#if cond`, `#elif cond`,
`#else`, `#endif`, `#define NAME`, `#undef NAME`. A condition is names and
values compared (`== != < <= > >=`, values by their dot-separated parts,
as numbers where they are), joined with `!`, `&&`, `||` and brackets. The
compiler declares the variables: `MC`, the game version a `.yns` is being
compiled for (once each version its `targets` takes). What a branch
leaves out isn't read at all on that version. The preprocessor is
general — it knows nothing of Yog or the game: `#if MC >= 1.21`.

## 2. Files and declarations

```ebnf
file        = using* namespace_decl? decl* ;

using       = 'global'? 'using' ('static'? path | IDENT '=' type) ';' ;
namespace_decl = 'namespace' path (';' | '{' using* decl* '}') ;
path        = IDENT ('.' IDENT)* ;
```

Namespaces and `using`s work as in C#:

- `namespace Charge.Machines;` puts the rest of the file in a namespace;
  `namespace Charge.Machines { … }` what's in its braces.
- `using System.Yog.Ui;` brings in the types of a namespace (not the namespaces
  in it); `using static System.Yog.Chat;` the static members of a type
  (`Broadcast(…)`); `using Grid = Dictionary<Pos, i32>;` names a type or a
  namespace; `global using …;` is every file's of the mod.
- A simple name is, in turn: a type of the namespace the code is in (or
  one enclosing it); an alias; a type a `using` brings in — where two
  bring one in, it's ambiguous (YS0022, C#'s CS0104): write it in full or
  alias it; then a namespace.
- Every namespace starts with `System`. `System.*` is the language's
  (`System`, `System.Meta`, `System.Collections.Generic`, …);
  `System.Yog.*` is Yog's own (its objects, groups and events,
  `System.Yog.Ui`, `System.Yog.Native`); `System.Game.*` is the game's
  classes, a `.yns` file's only (`using System.Game.World.Entity;`, or
  `System.Game.World.Entity.LivingEntity` in full).
- `System` and `System.Yog` are every file's `using`s (as .NET's implicit
  usings): Yog's objects need none.

```ebnf

decl        = attributes? doc? modifiers? ( mod_decl | method_decl | field_decl
            | type_decl | content_decl | component_decl | system_decl | extend_decl
            | screen_decl | command_decl | event_decl | test_decl
            | patch_decl | targets_decl ) ;      (* patch_decl, targets_decl: `.yns` only *)

modifiers   = modifier+ ;
modifier    = 'public' | 'private' | 'internal'         (* access; members are private, types internal (the mod) *)
            | 'protected'
            | 'static' | 'const' | 'readonly' | 'async'
            | 'abstract' | 'virtual' | 'override' | 'sealed' | 'new'   (* inheritance, as C#'s *)
            | 'extern'                                (* its body is elsewhere: on the bus, ys.md §7 *)
            | 'server' | 'client' | 'shared'          (* side, §4.1 of ys.md *)
            | 'saved' | 'synced' | 'tune' ;
attributes  = ('[' (attr_target ':')? attribute (',' attribute)* ']')+ ;   (* [Obsolete("…")] *)
attr_target = 'return' | 'module' | 'field' | 'property' | 'param' | 'type' ;
attribute   = path call_args? ;                     (* an attribute class, or an attribute macro *)
macro_decl  = 'macro' IDENT '{' macro_rule+ '}' ;   (* a declarative macro *)
macro_rule  = '(' token_tree* ')' '=>' '{' token_tree* '}' ';' ;
macro_call  = path '!' ('(' token_tree* ')' | '[' token_tree* ']' | '{' token_tree* '}') ;   (* no space before '!' *)
```

### 2.1 The mod

```ebnf
mod_decl    = 'mod' IDENT '{' (IDENT '=' expr ';' | member)* '}' ;   (* settings, then members; its constructor runs on load *)
```

```ys
mod Charge {
    Version = "1.0.0";
    Targets = mc 1.20.1..26.2;
    Resources = "charge.yres";
}
```

`Targets` takes a version range (`mc 1.20.1..26.2`, `mc >= 1.21`); a
resource file's entries are reachable as `Res.<Path>`. The mod's id is its
name in snake_case (`charge`) unless `Id = "…";` says otherwise.
`panic!`, `unreachable!`, `todo!` and `assert!` are `System` macros (as
Rust's), not keywords.

### 2.2 Methods, fields, properties

```ebnf
method_decl = type_or_void IDENT generics? '(' params? ')' constraints? requires? method_body ;
type_or_void = type | 'void' ;
method_body = block | '=>' expr ';' ;
params      = param (',' param)* ;
param       = pass? LIFETIME? type IDENT ('=' expr)? ;
pass        = 'in' | 'ref' | 'out' | 'move' ;                       (* ys.md §3 Memory *)
requires    = 'requires' expr ;                      (* a guard: commands, server methods called by clients *)
generics    = '<' IDENT (',' IDENT)* '>' ;
constraints = ('where' IDENT ':' type (',' type)*)+ ;

field_decl  = type IDENT bound? ('=' expr)? migration? ';'          (* a field; with `const`, a constant *)
bound       = 'in' range_expr ;                      (* i32 Level in 0..100: kept in range *)
            | type IDENT '=>' expr ';'                               (* a computed property *)
            | type IDENT '{' accessor+ '}' ('=' expr ';')?            (* a property with accessors *)
            | type 'this' '[' params ']' '{' accessor+ '}' ;          (* an indexer: what `x[i]` reads and writes *)
accessor    = ('get' | 'set' | 'init') (';' | method_body) ;
migration   = 'was' IDENT (':' type)? ;              (* an old save's name and type *)
```

```ys
i32 Area(i32 w, i32 h) => w * h;

void Greet(Player player) {
    player.Say("Hello, {player.Name}!");
}

T? Heaviest<T>(List<T> list) where T : Entity => list.MaxBy { it.Mass };
```

A method without a side modifier is `shared`. `server` and `client`
methods called on the other side are messages (`x.Client.F()`,
`x.Server.F()`, §3.6). `async` marks a method that awaits (§3.5).

### 2.3 Types of one's own

```ebnf
type_decl   = class_decl | struct_decl | record_decl | enum_decl | interface_decl | unit_decl ;
class_decl  = class_mod* 'class' IDENT generics? primary_ctor? bases? constraints? type_body ;
class_mod   = 'abstract' | 'sealed' | 'static' ;                    (* 'ref' struct: holds borrows *)
struct_decl = 'struct' IDENT generics? primary_ctor? bases? constraints? type_body ;
record_decl = 'record' ('class' | 'struct')? IDENT generics? primary_ctor bases? constraints? (type_body | ';') ;
enum_decl   = 'enum' IDENT generics? (':' type)? '{' variant (',' variant)* ','? (';' member*)? '}' ;
variant     = IDENT (primary_ctor | '=' expr)? ;                     (* Idle, Charging(i32 Rate), Wood = 1 *)
generics    = '<' generic (',' generic)* '>' ;
generic     = LIFETIME | ('in' | 'out')? IDENT ;                     (* 'a; variance: interfaces, delegates *)
constraints = ('where' IDENT ':' constraint (',' constraint)*)+ ;
constraint  = 'class' | 'struct' | 'new' '(' ')' | type ;
interface_decl = 'interface' IDENT generics? bases? '{' member* '}' ;
unit_decl   = 'unit' IDENT ('=' expr)? ';' ;                         (* a mod's own unit; `= 20t` defines it by another *)

primary_ctor = '(' params? ')' ;                                     (* the type's constructor, its parameters in scope *)
bases       = ':' type (',' type)* ;                                 (* a base class, then interfaces; content kinds (§2.4) *)
type_body   = '{' member* '}' ;
member      = attributes? doc? modifiers? (method_decl | field_decl | ctor_decl | handler) ;
ctor_decl   = IDENT '(' params? ')' (':' ('this' | 'base') call_args)? block ;  (* IDENT is the type's *)
```

```ys
record Pos(i32 X, i32 Y, i32 Z);

class Timer(Ticks every) {
    Ticks Left = every;
    public Timer() : this(20t) { }
}

enum Mode { Idle, Charging(i32 Rate), Full }

interface IChargeable {
    Energy Room { get; }
}
```

A record's primary constructor parameters are its public properties; a
class's and a component's are in scope in its body (C# 12's primary
constructors). A class's `:` names one base class and any interfaces; a
struct's and an interface's interfaces only; content's its kind's base
(§2.4). See ys.md §3 (Types) for what each kind of type is.

### 2.4 Content

```ebnf
content_decl = content_kind IDENT primary_ctor? bases? has? '{' content_item* '}' ;
content_kind = 'block' | 'item' | 'entity' | 'fluid' | 'recipe' | IDENT ;   (* open: a mod's own content kinds *)
has          = 'has' component_use (',' component_use)* ;
component_use = path call_args? ;                                    (* a component and its arguments *)
content_item = IDENT '=' expr ';'                                    (* a property: Hardness = 2.0f32; *)
             | member ;
```

```ys
block Charger has Charge(Res.Charger.Capacity) {
    Hardness = Res.Charger.Hardness;
    public Slots<Battery> Inventory = new(4);

    Charger() {
        Use += player => player.Open(new ChargerScreen(this));
    }
}

recipe Battery : Shaped {
    Pattern = ["RIR", "IGI", "RIR"];
    Keys = ['R' => @item:minecraft:redstone, 'I' => @item:minecraft:iron_ingot, 'G' => @item:minecraft:gold_ingot];
}
```

A content declaration is also a type (`Battery` is the item's type:
`b is Battery`, `new Battery()` a new stack). A recipe names its kind as
its base (`: Shaped`, `: Shapeless`, a mod's own kinds).

### 2.5 Components and systems

```ebnf
component_decl = 'component' IDENT primary_ctor? has? '{' content_item* '}' ;
system_decl    = modifiers? 'system' IDENT '(' params ')' ('every' expr)? block ;
extend_decl    = 'extend' type bases? has? ('{' member* '}' | ';') ;   (* added members (state too), interfaces, components *)
```

A component's constructor parameters are given where an object `has` it
(`has Charge(1000)`); its items are a content declaration's, its
constructor's subscriptions included (to the owner's events). A system's parameters are
what an object must have to be run over; their types are components,
object types or `&` of them. Per-scope state is `extend Player { … }`
(ys.md 4.2).

### 2.6 Screens and commands

```ebnf
screen_decl  = 'screen' IDENT primary_ctor? ':' 'Ui' '(' STRING ')' type_body ;
command_decl = 'command' STRING '{' member* '}' ;   (* each method a subcommand; its parameters the arguments *)
```

A screen's `server` methods are its actions: a client calls them (a
message).

### 2.6.1 Events

Events are C#'s. A type declares one (`public event Action<Player, int>
Charged;`); code subscribes with `+=` — a lambda, whose parameters take
the event's types, or a method of its signature — leaves with `-=`, and
raises it with `Charged?.Invoke(player, 3);` (or `Charged(player, 3)`).
Yog's events are its groups' (`Player.Join`, `Server.Tick`,
`Block.Break`), their argument each a `…Args` of that group
(`Player.JoinArgs`: `e.PlayerName`, `e.Dimension`). Subscriptions are made
where code runs when things start, as in C#: the mod's constructor (it runs
when the mod loads), a content's or a component's.

```ys
mod Charge {
    Version = "1.0.0";

    Charge() {
        Player.Join += e => Chat.Broadcast("welcome, {e.PlayerName}");
        Server.Tick += OnTick;
    }

    server void OnTick(Server.TickArgs e) { … }
}
```

### 2.7 Events, tests, native

```ebnf
event_member = modifiers? 'event' type IDENT ';' ;                    (* public event Action<Player, i32> Charged; *)
test_decl    = 'test' STRING block ;
version_cond = 'mc' ( ('>=' | '<=' | '>' | '<' | '==') VERSION | VERSION '..' VERSION ) ;
(* Rust code is reached through the bus: `[Service("addr")] static extern R Name(params);`, ys.md §7 *)
```

## 3. Statements and expressions

### 3.1 Blocks and statements

```ebnf
block       = '{' stmt* '}' ;
stmt        = local_decl ';' | expr ';' | assign ';' | block
            | if_stmt | foreach_stmt | for_stmt | while_stmt | do_stmt
            | 'break' ';' | 'continue' ';' | 'return' expr? ';' | 'skip' expr? ';' ;
local_decl  = 'let' 'mut'? binding (':' type)? ('=' expr)?
            | type IDENT ('=' expr)? ;                               (* C#'s typed local: assignable *)
binding     = IDENT | '(' IDENT (',' IDENT)+ ')' ;                   (* deconstruction *)
assign      = place assign_op expr ;
assign_op   = '=' | '+=' | '-=' | '*=' | '/=' | '%=' | '??=' ;
place       = postfix ;                                              (* a name, a member, an index *)

if_stmt     = 'if' '(' expr ')' stmt ('else' stmt)? ;
foreach_stmt = 'foreach' '(' ('let' | type) (IDENT | '(' IDENT (',' IDENT)+ ')') 'in' expr ')' stmt ;
for_stmt    = 'for' '(' local_decl? ';' expr? ';' assign? ')' stmt ;
while_stmt  = 'while' '(' expr ')' stmt ;
do_stmt     = 'do' stmt 'while' '(' expr ')' ';' ;
```

`if` takes parentheses, as in C#; a body without braces is one statement.
The conditional expression is `c ? a : b`; `switch` is an expression
(§3.3).

### 3.2 Expressions by precedence (low to high)

| Level | Operators | Assoc. |
|---|---|---|
| 1 | `=>` lambdas | right |
| 2 | `c ? a : b` | right |
| 3 | `x ?? y` | right |
| 4 | `\|\|` | left |
| 5 | `&&` | left |
| 6 | `==` `!=` | left |
| 7 | `<` `<=` `>` `>=` `is` `as` `has` `in` | left |
| 8 | `..` `..<` (ranges) | none |
| 9 | `+` `-` | left |
| 10 | `*` `/` `%` | left |
| 11 | `switch` (`x switch { … }`) | left |
| 12 | prefix `-` `!` `await` `try` `(T)` cast | right |
| 13 | postfix: call `f(…)`, index `a[i]`, member `.M`, null-conditional `?.M`, `!` (not null), trailing lambda `{ … }`, side `.Server` / `.Client` | left |

```ebnf
expr        = lambda | conditional ;
primary     = literal | macro_call | IDENT | 'this' | 'it' | 'result' | 'value' | 'null'
            | '(' expr ')' | tuple | collection | new_expr | block_lambda ;
literal     = INT | FLOAT | UNIT | STRING | RAW_STRING | CHAR | TEXT | CONTENT | TAG | 'true' | 'false' ;
tuple       = '(' expr ',' expr (',' expr)* ')' ;
collection  = '[' (element (',' element)* ','?)? ']' ;            (* a list, a set or a map, by its target type *)
element     = expr ('=>' expr)? | '..' expr ;                     (* k => v in a map; ..xs spreads *)
new_expr    = 'new' type? call_args? initializer? ;                (* new Pos(1, 2, 3); new(4) where the type is known *)
initializer = '{' (IDENT '=' expr (',' IDENT '=' expr)* ','?)? '}' ;   (* new Battery { Count = 2 } *)
lambda      = (IDENT | '(' (param_or_name (',' param_or_name)*)? ')') '=>' (expr | block) ;
param_or_name = type? IDENT ;
block_lambda = '{' stmt* expr? '}' ;                               (* trailing lambda: `it` its parameter, the last expression its value *)
call_args   = '(' (arg (',' arg)*)? ')' ;
arg         = (IDENT ':')? pass? (out_decl | expr) ;               (* named arguments, as in C# *)
out_decl    = ('let' 'mut'? | type) IDENT ;                  (* F(out let x) *)
```

`list.Select(x => x * 2)` and `list.Select { it * 2 }` are the same call:
a trailing block lambda is the last argument, `it` its one parameter.

### 3.3 Switch and patterns

```ebnf
switch_expr = postfix 'switch' '{' arm (',' arm)* ','? '}' ;
arm         = pattern ('when' expr)? '=>' expr ;
pattern     = '_' | literal | 'let' IDENT | type_pat | variant_pat | relational_pat | range_pat
            | tuple_pat | property_pat | pattern 'or' pattern | pattern 'and' pattern | 'not' pattern ;
type_pat    = type IDENT? ;                                         (* Monster m *)
variant_pat = path '(' pattern (',' pattern)* ')' ;                 (* Mode.Charging(let rate) *)
relational_pat = ('<' | '<=' | '>' | '>=') literal ;
range_pat   = literal '..' literal ;
tuple_pat   = '(' pattern (',' pattern)* ')' ;
property_pat = type? '{' IDENT ':' pattern (',' IDENT ':' pattern)* '}' ;   (* { Health: < 5 } *)
is_expr     = expr 'is' pattern ;                                   (* if (target is Monster m && m.IsAlive) *)
```

A `switch` over an enum or a sealed class's subclasses must cover every
case (or `_`): a case left out is an error.

### 3.4 Nulls and failures

- `T?` may be `null`; other types can't be. `a?.B` is `null` if `a` is;
  `a ?? b` the fallback; `a!` asserts it isn't (a build warning where it
  can't be proven); `is` patterns narrow (`if (target is Player p)`).
- A method returning `Result<T, E>` (`Ok(v)` / `Err(e)`): `try F()` is
  `v`, or returns the `Err` from the caller (converted to its `E`), as
  Rust's `F()?`. A `Result` left unused is a warning.
- A bug panics (`panic!(…)`, `Unwrap()` of an `Err`/`null`, `unreachable!()`,
  an index out of range). There are no exceptions to catch: a panic
  unwinds to its handler's boundary (an event handler, a task, a patch),
  which reports it with its `.ys` line (ys.md §4.5); the game goes on.
  `Panic.SetHook` sees panics, `Panic.Catch` turns one into a `Result`.

### 3.5 Time and queries are library

Waiting, periods and world searches are the standard library (`Task`,
`Level`), awaited as in C#:

```ys
server async void Arm(Charger charger) {
    charger.Glow();
    await Task.Delay(5s);                // the game goes on meanwhile
    charger.Explode(power: 2.0f32);
}

Task.Every(10s, () => { … });            // a period
Task.Every(10s) { … }                    // the same, a trailing lambda
await Task.Until { door.IsOpen };        // a condition
let near = Level.Entities.Near(spawn, 32b).Where { it is Monster }.OrderBy { it.Distance }.Take(5);
```

- An `async` method is compiled into a state machine run by its side's
  ticks; calling it gives a `Task` (awaited or left to run).
- A task started from a `saved` object is saved with it when everything it
  captured is savable; otherwise the build warns that it won't survive a
  restart.
- `await` in a `shared` method is an error (whose ticks?).
- `Near(…)` and its chain are known to the compiler: they compile to the
  game's indexed lookups (chunk and entity sections), not a scan.

### 3.6 Across sides

`x.Client.F(args)` calls `client` method `F` on the client(s) of `x` (a
player: that one; a block or entity: the clients tracking it);
`x.Server.F(args)` calls `server` method `F` from a client. Both compile
to a generated message. A `server` method taking a first parameter of
type `Sender` receives the caller.

## 4. Types

```ebnf
type        = base_type ('&' base_type)* '?'? ;                     (* Charge & Owned: an object with both *)
base_type   = path generic_args?                                     (* i32, List<T>, LivingEntity, a component *)
            | ('in' | 'ref') LIFETIME? base_type                     (* a borrow: in 'a T is &'a T, ref 'a T &'a mut T *)
            | LIFETIME ;                                             (* among type arguments: Cursor<'a> *)
            | '(' type IDENT? (',' type IDENT?)+ ')' ;               (* a tuple *)
generic_args = '<' type (',' type)* '>' ;
```

Built in, named as Rust's: `i8` `i16` `i32` `i64` `u8` `u16` `u32` `u64` `f32` `f64`, and `decimal` `char` `bool` `string` `object` `dynamic`; a literal takes one as a suffix (`5u8`, `2.0f32`)
`string` `Text` `Ticks` `Seconds` `Blocks` `List<T>` `Dictionary<K, V>`
`HashSet<T>` `Func<…>` `Action<…>` `Expression<…>` `Result<T, E>` `Task` `Sender`
`Slots<T>`, content refs (`ItemRef`, `BlockRef`, `TagRef`, … one per
kind), and Yog's objects (`Player`, `Level`, `Entity`, `Block`,
`ItemStack`, …). In a `.yns` only, the game's classes are types by their
Mojang names (`LivingEntity`; a nested one with its outer class,
`OutgoingChatMessage.Player`); their side comes from the class index.

## 5. Collections and lambdas

LINQ's names, on every collection and on the world's queries:

```ys
let names = players.Where { it.Level > 10 }.Select { it.Name };
foreach (let battery in Inventory.OfType<Battery>()) battery.Charge.Fill();
let total = stacks.Sum(s => s.Count);
List<i32> empty = [];
Dictionary<string, i32> scores = ["alex" => 3, "sam" => 5];
```

## 6. Patches (`.yns` only)

A `.yns` says the game versions it's for, once, before anything else;
the build checks every patch on each of them:

```ebnf
targets_decl = 'targets' version_cond ';' ;           (* targets mc 1.20.1..26.2; *)
```

```ebnf
patch_decl  = 'patch' type '{' patch_item* '}' ;
patch_item  = 'add' modifiers? (field_decl | method_decl)            (* added members *)
            | 'add' bases ';'                                          (* added interfaces *)
            | 'before' method uses? block                              (* @Inject HEAD *)
            | 'after' method uses? block                               (* @Inject TAIL / @ModifyReturnValue via result *)
            | 'inside' method '{' point* '}'
            | 'read' IDENT block                                       (* @Redirect FIELD get *)
            | 'write' IDENT block                                      (* @Redirect FIELD put; `value` the new one *)
            | 'replace' method block                                   (* @Overwrite *)
            | 'priority' INT ';' ;
method      = type_or_void IDENT '(' params? ')' ;                    (* the target by signature *)
uses        = 'uses' local (',' local)* ;
local       = 'mut'? 'local' type IDENT ('#' INT)? ;                   (* by name; #n: the n-th of its type *)
point       = 'around' 'call' callee selector? block                   (* @WrapOperation *)
            | 'at' 'call' callee selector? '(' IDENT ')' block         (* @ModifyArg: the argument by name *)
            | 'before' 'call' callee selector? uses? block             (* @Inject INVOKE *)
            | 'after' 'call' callee selector? uses? block              (* @Inject INVOKE, shift AFTER *)
            | 'at' ('read' | 'write') IDENT selector? uses? block      (* @Inject FIELD *)
            | 'at' 'new' type selector? uses? block                    (* @Inject NEW *)
            | 'constant' literal '=>' expr selector? ';'               (* @ModifyConstant *)
            | 'local' type IDENT ('#' INT)? '=' expr ';' ;             (* @ModifyVariable *)
callee      = path ('(' (type (',' type)*)? ')')? ;                   (* AiStep(), Vec3.Add *)
selector    = 'first' | 'last' | 'where' '(' expr ')' ;
```

In a patch body:

- `this` is the patched object; its members are reachable unqualified
  (`Shield`, `Level`); the method's parameters by their names.
- `result` (in `after`) is the method's value, assignable.
- `skip expr?;` (in `before`) ends the method with that value; falling
  through runs it.
- `proceed(args?)` (in `around`, `read`, `write`) is the original; not
  calling it skips it.
- A callee or field that matches more than one point without a
  `selector` is a build error naming the candidates.

## 7. One file, every form

```ys
using System.Yog.Ui;

mod Charge {
    Version = "1.0.0";
    Targets = mc 1.20.1..26.2;
    Resources = "charge.yres";
}

unit Energy;                                 // a mod's own unit: 100Energy

record Link(Pos From, Pos To);

enum Mode { Idle, Charging(i32 Rate), Full }

component Charge(Energy capacity) {
    public saved synced Energy Energy in 0..capacity = 0 was Power: i32;
    public Energy Capacity => capacity;
    public Energy Room => capacity - Energy;

    public void Push(Charge into) {
        let moved = Math.Min(Energy, into.Room);
        into.Energy += moved;
        Energy -= moved;
    }
}

extend Player {
    public saved i32 Visits = 0;
    public bool IsCharged => Inventory.Any { it is Battery b && b.Charge.Energy > 0 };
}

block Charger has Charge(Res.Charger.Capacity) {
    Hardness = Res.Charger.Hardness;
    Drops = this;

    public saved Mode Mode = Mode.Idle;
    public Slots<Battery> Inventory = new(4);
    public tune i32 Rate = 4;

    Charger() {
        Tick += Charge;
        Use += player => player.Open(new ChargerScreen(this));
    }

    server void Charge() {
        foreach (let battery in Inventory) Charge.Push(battery.Charge);
        Mode = Charge.Energy switch {
            0 => Mode.Idle,
            let e when e == Charge.Capacity => Mode.Full,
            _ => new Mode.Charging(Rate),
        };
    }
}

server system Solar(Charge charge, Block block) every 1t {
    if (block.Level.IsDay) charge.Energy += Res.SolarRate;
}

item Battery has Charge(Res.Battery.Capacity) {
    StackSize = 1;
    Tooltip = t"charge.battery.tooltip"(Charge.Energy, Charge.Room);
}

recipe Battery : Shaped {
    Pattern = ["RIR", "IGI", "RIR"];
    Keys = ['R' => @item:minecraft:redstone, 'I' => @item:minecraft:iron_ingot, 'G' => @item:minecraft:gold_ingot];
}

client void Welcome(i32 times) {
    Toast(t"charge.welcome"(times));
}

server void Welcomed(Player.JoinArgs e) {
    let player = new Player(e.PlayerName);
    player.Visits += 1;
    player.Client.Welcome(player.Visits);
    if (player.Visits == 1) player.Give(new Battery());
}

server void Started(Server.StartedArgs e) {
    Task.Every(10s) {
        let lonely = Level.Entities.Near(Spawn, 32b).Where { it is Monster }.OrderBy { it.Distance }.Take(5);
        foreach (let monster in lonely) monster.Glow(5s);
    };
}

screen ChargerScreen(Charger block) : Ui("charger.yui") {
    Energy Energy => block.Charge.Energy;
    f32 Fill => Energy / block.Charge.Capacity;
    server void Dump() { block.Charge.Energy = 0; }
}

command "charge" {
    server void Fill(Sender source, Player target = source.Player) requires source.IsOp {
        foreach (let battery in target.Inventory.OfType<Battery>()) battery.Charge.Energy = battery.Charge.Capacity;
    }
}

test "a charger fills a battery" {
    let world = TestWorld();
    let charger = world.Place<Charger>(Origin);
    charger.Charge.Energy = 100;
    let battery = charger.Inventory.Put(new Battery());
    world.Tick(1t);
    Expect(battery.Charge.Energy == 100 && charger.Charge.Energy == 0);
}
```

```yns
targets mc 1.20.1..26.2;

using System.Game.World.Entity;              // LivingEntity
using System.Game.Server;                    // MinecraftServer
using Player = System.Game.World.Entity.Player.Player;   // the game's (Yog's Player is every file's)

patch LivingEntity {
    add public saved f32 Shield = 0;
    add : IChargeable;

    before bool Hurt(DamageSource source, f32 amount) {
        if (Shield >= amount) {
            Shield -= amount;
            skip false;
        }
    }

    after f32 GetMaxHealth() { result += Shield / 2; }

    inside void Tick() {
        around call AiStep() { if (!Level.IsFrozen) proceed(); }
    }

    inside void Knockback(f64 strength, f64 x, f64 z) {
        at call Vec3.Add first (x) { x * 0.5 }
        constant 0.4 => 0.2;
    }
}

patch MinecraftServer {
    after void TickServer(BooleanSupplier hasTime) uses local i64 start, mut local i64 spent #1 {
        if (spent > Res.Limits.SlowTick) Log.Warn("slow tick: {spent} ns");
    }
}

patch Player {
    read FoodData {
        return HasEffect(@effect:charge:overcharged) ? FoodData.Full : proceed();
    }
    write ExperienceLevel {
        if (value >= ExperienceLevel) proceed(value);
    }
    replace bool CanEat(bool ignoreHunger) {
        return ignoreHunger || FoodData.NeedsFood() || (this as IChargeable)?.Shield > 0;
    }
    priority 100;
}
```

## 8. Conversions

Implicit only where a conversion is **lossless and always possible**; one
that can fail is `as` (giving `null`) or a member giving a nullable; a
lossy one is an explicit cast or call.

| From → to | How | Why |
|---|---|---|
| `Block` → item | `block.Item: Item?` | fire, water, portals have no item |
| `Item` → block | `item.Block: Block?` | only block items have one |
| `Block` → `BlockState` | implicit (its default state); back: `state.Block` | one default, no loss |
| `Item` → `ItemStack` | implicit (one); back: `stack.Item` | a stack's count and components are lost going back |
| `Fluid` ↔ `Block` | `fluid.Block: Block?`, `block.Fluid: Fluid?` | not every fluid has a block |
| content ↔ tag | never; `x in #tag` | a tag is a set |
| `Seconds` → `Ticks` | implicit (×20, exact) | |
| `Ticks` → `Seconds` | explicit `t.Seconds` (fractional) | `7t` is 0.35 s |
| `Ticks` ↔ `TimeSpan` | never implicit | game time vs wall time: `/tick rate` and lag make 20 t ≠ 1 s |
| `Blocks` → `f64` | implicit; back explicit | a bare number could be anything |
| `i32` → `i64` / `f32` / `f64` | implicit (as C#) | widening |
| `f32` → `i32` | `(i32)x` truncates, or `Math.Floor`, `Math.Round` | coordinates need `Floor`: −0.5 is block −1 |
| `Vec3` → `Pos` | explicit `v.BlockPos` (floor); `Pos` → `Vec3`: `pos.Center` | |
| `i32` → `i32 in a..b` | a literal checked at build; an expression: `Math.Clamp(…)` or `as` | a clamp is visible |
| `string` → `Text` | implicit (literal text) | |
| `Text` → `string` | `text.Resolve()`, `client` only | it needs the client's language |
| a mod's units | declared conversions only (`unit FE = Energy * 1;`); different quantities never mix | |
| `Entity` → subtype | `as` → nullable; `is` patterns narrow | a downcast can fail |
| another mod's content | `@item:create:wrench` is `Item?` unless that mod is required | it may be absent |

Edge cases: a stack held after it's used up is an empty stack (as in the
game), not `null`; an entity held in a `saved` field reads as `null` once
removed or unloaded (its type is always `Entity?` there); content missing
on a targeted version is a diagnostic for that version, used only inside
`version` blocks.

## 9. Generics

Constraints as in C#: `where T : Base` (a class, an interface or a
component).

```ys
T? Heaviest<T>(List<T> list) where T : Entity => list.MaxBy { it.Mass };
```
