# Yog Script — grammar

Status: design draft, the syntax of `docs/design/ys.md` made exact. `.ys`
and `.yns` share this grammar; the patch forms (§6) are accepted only in
`.yns`.

Notation: EBNF. `a?` optional, `a*` zero or more, `a+` one or more,
`( … )` grouping, `|` choice, `'x'` a literal token, `UPPER` a lexical token.

## 1. Lexical

### 1.1 Layout

- Source is UTF-8. Whitespace separates tokens; indentation means nothing.
- A **newline ends a statement** unless the line ends with a binary
  operator, `,`, `.`, `(`, `[`, `{`, `->`, `=>`, `=`, or the next line
  starts with `.` or `?.` (a chain continues). `;` ends a statement
  explicitly (several on one line).
- Comments: `// …` to the end of the line, `/* … */` (nesting), `/// …` a
  doc comment for the next declaration (shown by the LSP and IDLE).

### 1.2 Tokens

```ebnf
IDENT      = letter (letter | digit | '_')* ;        (* not a keyword *)
INT        = digit (digit | '_')* | '0x' hex+ | '0b' ('0'|'1')+ ;
FLOAT      = digit+ '.' digit+ (('e'|'E') ('+'|'-')? digit+)? ;
UNIT       = (INT | FLOAT) unit_suffix ;
unit_suffix = 't' | 's' | 'ms' | 'm' | 'h' | 'b' | IDENT ;   (* t ticks, s seconds, b blocks; a mod's own units by name *)
STRING     = '"' (char | '\' escape | '{' expr '}')* '"' ;   (* {expr} interpolates; {{ }} are braces *)
RAW_STRING = 'r"' char* '"' ;
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
mod use fn let var val const type unit record union enum extend
if else match for in while loop break continue return try
true false none self it is as and or not
server client shared on skip
saved synced was tune test version native
```

Contextual (keywords only where the grammar expects them, names
elsewhere): `block item entity fluid recipe screen command state player
world session ui action slots of where first last
requires patch before after inside around call read write constant
replace add uses local proceed result shaped shapeless`.

## 2. Files and declarations

```ebnf
file        = use* decl* ;

use         = 'use' path ('.' '{' IDENT (',' IDENT)* '}' | 'as' IDENT)? ;
path        = IDENT ('.' IDENT)* ;

decl        = doc? modifiers? ( mod_decl | fn_decl | let_decl | var_decl | const_decl
            | unit_decl | recipe_decl | type_decl | record_decl | union_decl | enum_decl | extend_decl
            | content_decl | state_decl | screen_decl | command_decl
            | event_decl | test_decl | version_decl | native_decl
            | patch_decl ) ;

modifiers   = modifier+ ;
modifier    = 'server' | 'client' | 'shared'          (* side, §4.1 of ys.md *)
            | 'saved' | 'synced' | 'tune' | 'pub' ;
```

### 2.1 The mod

```ebnf
mod_decl    = 'mod' IDENT '{' (IDENT '=' expr)* '}' ;
```

```ys
mod charge {
    version = "1.0.0"
    targets = mc 1.20.1..26.2
    resources = "charge.yres"
}
```

`targets` takes a version range (`mc 1.20.1..26.2`, `mc >= 1.21`); a
resource file's entries are reachable as `res.<path>`.

### 2.2 Values and functions

```ebnf
let_decl    = 'let' pattern (':' type)? '=' expr ;
var_decl    = 'var' IDENT (':' type)? ('=' expr)? migration? ;
const_decl  = 'const' IDENT (':' type)? '=' expr ;
val_decl    = 'val' IDENT (':' type)? '=>' expr ;    (* computed on every read *)
migration   = 'was' IDENT (':' type)? ;              (* an old save's name and type *)

fn_decl     = 'fn' IDENT generics? '(' params? ')' ('->' type)? (fn_body | requires fn_body) ;
fn_body     = block | '=' expr ;
params      = param (',' param)* ;
param       = 'var'? IDENT ':' type ('=' expr)? ;
requires    = 'requires' expr ;                      (* a guard: commands, server fns called by clients *)
generics    = '<' IDENT (':' type)? (',' IDENT (':' type)?)* '>' ;
```

A function without a side modifier is `shared`. `server fn` and
`client fn` called on the other side are messages (`x.client.f()`,
`x.server.f()`, §3.6).

### 2.3 Types of one's own

```ebnf
type_decl   = 'type' IDENT generics? '=' type ;                       (* an alias *)
unit_decl   = 'unit' IDENT ('=' expr)? ;                             (* a mod's own unit; `= 20t` defines it by another *)
record_decl = 'record' IDENT generics? '(' params ')' type_body? ;
union_decl  = 'union' IDENT generics? '{' variant (',' variant)* ','? member* '}' ;
variant     = IDENT ('(' params ')')? ;
enum_decl   = 'enum' IDENT '{' IDENT (',' IDENT)* ','? member* '}' ;
extend_decl = 'extend' type (':' type)? '{' member* '}' ;            (* added members, an added interface *)
type_body   = '{' member* '}' ;
member      = doc? modifiers? (fn_decl | let_decl | var_decl | val_decl | const_decl) ;
```

### 2.4 Content

```ebnf
content_decl = content_kind IDENT (':' type)? '{' content_item* '}' ;
content_kind = 'block' | 'item' | 'entity' | 'fluid' | IDENT ;       (* open: a mod's own content kinds *)
content_item = IDENT '=' expr                                         (* a property: hardness = 2.0 *)
             | modifiers? (var_decl | val_decl | let_decl | fn_decl)
             | slots_decl | handler ;
slots_decl   = 'slots' IDENT '[' expr ']' ('of' type)? ;
handler      = modifiers? 'on' IDENT ('(' params_untyped? ')')? block ; (* on use(p), on tick, on load *)
params_untyped = IDENT (',' IDENT)* ;

recipe_decl  = 'recipe' type recipe_kind '{' recipe_body '}' ;
recipe_kind  = 'shaped' | 'shapeless' | IDENT ;                       (* open: a mod's own recipe kinds *)
recipe_body  = STRING* (IDENT '=' expr)* ;                            (* rows, then keys *)
```

`recipe_decl` is a `content_decl` form. A content declaration is also a
type (`Battery` is the item's type: `b is Battery`, `Battery()` a new
stack).

### 2.5 State in a scope

```ebnf
state_decl  = scope 'state' '{' member* '}' ;
scope       = 'player' | 'world' | 'session' | 'level' ;
```

`p.state.visits` reads a player's; `world.state.grid` the world's.

### 2.6 Screens and commands

```ebnf
screen_decl  = 'screen' IDENT '(' params? ')' 'ui' STRING '{' screen_item* '}' ;
screen_item  = val_decl | var_decl | fn_decl
             | 'action' IDENT '(' params? ')' fn_body ;              (* runs on the server: a message *)

command_decl = 'command' STRING '{' (modifiers? fn_decl)* '}' ;      (* each fn a subcommand; its params the arguments *)
```

### 2.7 Events, tests, versions, native

```ebnf
event_decl   = modifiers? 'on' path ('(' params_untyped? ')')? block ;  (* on player.join(p) *)
test_decl    = 'test' STRING block ;
version_decl = 'version' version_cond '{' decl* '}' ('else' (version_decl | '{' decl* '}'))? ;
version_cond = 'mc' ( ('>=' | '<=' | '>' | '<' | '==') VERSION | VERSION '..' VERSION ) ;
native_decl  = 'native' STRING '{' fn_sig* '}' ;
fn_sig       = 'fn' IDENT '(' params? ')' ('->' type)? ;
```

## 3. Statements and expressions

### 3.1 Blocks and statements

```ebnf
block       = '{' stmt* '}' ;
stmt        = let_decl | var_decl | assign | if_stmt | match_expr | for_stmt
            | while_stmt | loop_stmt | 'break' | 'continue'
            | 'return' expr? | 'skip' expr?
            | expr ;
assign      = place assign_op expr ;
assign_op   = '=' | '+=' | '-=' | '*=' | '/=' | '%=' | '??=' ;
place       = postfix ;                                      (* a name, a member, an index *)

if_stmt     = 'if' expr block ('else' (if_stmt | block))? ;
for_stmt    = 'for' pattern 'in' expr block ;
while_stmt  = 'while' expr block ;
loop_stmt   = 'loop' block ;
```

`if` and `match` are expressions too: `let tier = if x { 1 } else { 2 }`.
Conditions take no parentheses; bodies always take braces.

### 3.2 Expressions by precedence (low to high)

| Level | Operators | Assoc. |
|---|---|---|
| 1 | `x ?? y` | right |
| 2 | `or` / `\|\|` | left |
| 3 | `and` / `&&` | left |
| 4 | `==` `!=` `<` `<=` `>` `>=` `is` `!is` `in` `!in` | none |
| 5 | `..` `..<` (ranges) | none |
| 6 | `+` `-` | left |
| 7 | `*` `/` `%` | left |
| 8 | `as` `as?` (conversion) | left |
| 9 | prefix `-` `not` `!` `try` | right |
| 10 | postfix: call `f(…)`, index `a[i]`, member `.m`, safe member `?.m`, trailing lambda `{ … }`, side `.server` / `.client` | left |

```ebnf
expr        = binary ;
primary     = literal | IDENT | 'self' | 'it' | 'result' | 'none'
            | '(' expr ')' | tuple | list | map | lambda
            | if_stmt | match_expr | block ;
literal     = INT | FLOAT | UNIT | STRING | RAW_STRING | TEXT | CONTENT | TAG | 'true' | 'false' ;
tuple       = '(' expr ',' (expr (',' expr)*)? ')' ;
list        = '[' (expr (',' expr)* ','?)? ']' ;
map         = '{' (expr ':' expr (',' expr ':' expr)* ','?)? '}' ;   (* {} is an empty map where a value is expected *)
lambda      = '{' (params_untyped '->')? stmt* '}' ;                   (* no params: `it` *)
call_args   = '(' (arg (',' arg)*)? ')' ;
arg         = (IDENT '=')? expr ;                                      (* named arguments *)
```

### 3.3 Match and patterns

```ebnf
match_expr  = 'match' expr '{' arm+ '}' ;
arm         = pattern ('if' expr)? '->' (expr | block) ;
pattern     = '_' | literal | IDENT | range_pat | type_pat | variant_pat
            | tuple_pat | pattern '|' pattern | 'else' ;
type_pat    = 'is' type (IDENT)? ;                    (* is Monster m *)
variant_pat = path '(' pattern (',' pattern)* ')' ;   (* Circle(r) *)
range_pat   = literal '..' literal ;
tuple_pat   = '(' pattern (',' pattern)* ')' ;
```

A `match` over a union or enum must cover every case (or `else`).

### 3.4 Optionals and failures

- `T?` may be `none`. `a?.b` is `none` if `a` is; `a ?? b` the fallback;
  `if let p = target { … }` and `match` unwrap.
- A function returning `Result<T>` (`Ok(v)` / `Fail(reason)`): `try f()`
  is `v`, or returns the failure from the caller. In a handler (`on`,
  a task, a patch), an unhandled failure stops that handler and is
  reported with its `.ys` line (ys.md §4.5); the game goes on.

### 3.5 Time and queries are library, not syntax

Waiting, periods and world searches are the standard library (`Task`,
`Level`), not words of the language:

```ys
server fn arm(c: Charger) {
    c.glow()
    Task.wait(5s)                    // suspends this function; the game goes on
    c.explode(power = 2.0)
}

Task.every(10s) { … }                // a period
Task.delay(5s) { … }                 // once, later
Task.until { door.isOpen }           // a condition
let near = level.entities.near(spawn, 32b).where { it is Monster }.orderBy { it.distance }.take(5)
```

- A function calling `Task.wait` / `Task.until` (or another such function)
  is **suspending**: the compiler turns it into a state machine run by its
  side's ticks; no `async` / `await` words. Calling it gives a `Task`:
  `task.wait()` waits for it (making the caller suspending too), otherwise
  it runs on its own.
- A task started from a `saved` object is saved with it when everything it
  captured is savable; otherwise the build warns that it won't survive a
  restart.
- `Task.wait` in a `shared` function is an error (whose ticks?).
- `near(…)` and its chain are known to the compiler: they compile to the
  game's indexed lookups (chunk and entity sections), not a scan.

### 3.6 Across sides

`x.client.f(args)` calls `client fn f` on the client(s) of `x` (a player:
that one; a block or entity: the clients tracking it); `x.server.f(args)`
calls `server fn f` from a client. Both compile to a generated message.
A `server fn` taking a first parameter of type `Sender` receives the
caller.

## 4. Types

```ebnf
type        = base_type ('?')? ;
base_type   = path generic_args?                                 (* Int, List<T>, LivingEntity *)
            | base_type 'in' range_expr                          (* bounded: Int in 0..100 *)
            | '(' type (',' type)+ ')'                           (* a tuple *)
            | '(' (type (',' type)*)? ')' '->' type ;            (* a function *)
generic_args = '<' type (',' type)* '>' ;
```

Built in: `Int` `Int32` `Byte` `Float` `Double` `Bool` `Str` `Text`
`Ticks` `Seconds` `Blocks` `List<T>` `Map<K,V>` `Set<T>` `Result<T>`
`Sender`, content refs (`ItemRef`, `BlockRef`, `TagRef`, … one per kind).
The game's classes are types by their Mojang names (`LivingEntity`,
`net.minecraft.world.entity.LivingEntity` when ambiguous); their side
comes from the class index.

## 5. Generic forms

Collections, lambdas and trailing lambdas:

```ys
let names = players.where { it.level > 10 }.map { it.name }
inventory.each<Battery> { it.charge = it.charge.max }
let total = stacks.sum { it.count }
```

## 6. Patches (`.yns` only)

```ebnf
patch_decl  = 'patch' type '{' patch_item* '}' ;
patch_item  = 'add' modifiers? (var_decl | val_decl | fn_decl)         (* added members *)
            | 'add' 'is' type                                            (* an added interface *)
            | 'before' method uses? block                                (* @Inject HEAD *)
            | 'after' method uses? block                                 (* @Inject TAIL / @ModifyReturnValue via result *)
            | 'inside' method '{' point* '}'
            | 'read' IDENT block                                         (* @Redirect FIELD get *)
            | 'write' IDENT '(' IDENT ')' block                          (* @Redirect FIELD put *)
            | 'replace' method block                                     (* @Overwrite *)
            | priority ;
method      = IDENT '(' params? ')' ('->' type)? ;                      (* the target by signature *)
uses        = 'uses' local (',' local)* ;
local       = 'var'? 'local' IDENT ':' type ('#' INT)? ;                 (* by name; #n: the n-th of its type *)
point       = 'around' 'call' callee selector? block                     (* @WrapOperation *)
            | 'at' 'call' callee selector? '(' IDENT ')' block           (* @ModifyArg: the argument by name *)
            | 'before' 'call' callee selector? uses? block               (* @Inject INVOKE *)
            | 'after' 'call' callee selector? uses? block                (* @Inject INVOKE, shift AFTER *)
            | 'at' ('read' | 'write') IDENT selector? uses? block        (* @Inject FIELD *)
            | 'at' 'new' type selector? uses? block                      (* @Inject NEW *)
            | 'constant' literal '->' expr selector?                     (* @ModifyConstant *)
            | 'local' IDENT ':' type ('#' INT)? '=' expr ;               (* @ModifyVariable *)
callee      = path ('(' (type (',' type)*)? ')')? ;                     (* aiStep(), Vec3.add *)
selector    = 'first' | 'last' | 'where' expr ;
priority    = 'priority' INT ;
```

In a patch body:

- `self` is the patched object; its members are reachable unqualified
  (`shield`, `level`); the method's parameters by their names.
- `result` (in `after`) is the method's value, assignable.
- `skip expr?` (in `before`) ends the method with that value; falling
  through runs it.
- `proceed(args?)` (in `around`, `read`, `write`) is the original; not
  calling it skips it.
- A callee or field that matches more than one point without a
  `selector` is a build error naming the candidates.

## 7. One file, every form

```ys
use yog.ui

mod charge {
    version = "1.0.0"
    targets = mc 1.20.1..26.2
    resources = "charge.yres"
}

unit Energy                              // a mod's own unit: 100Energy

record Link(from: Pos, to: Pos)

union Mode { Idle, Charging(rate: Int), Full }

extend Player {
    val isCharged: Bool => inventory.any { it is Battery b and b.charge > 0 }
}

block Charger {
    hardness = res.charger.hardness
    drops = self

    saved synced var energy: Energy in 0..res.charger.capacity = 0 was power: Int
    saved var mode: Mode = Idle
    slots inventory[4] of Battery
    tune let rate = 4

    server on tick {
        if level.isDay { energy += rate }
        for b in inventory {
            let moved = min(energy, b.room)
            b.charge += moved
            energy -= moved
        }
        mode = match energy {
            0 -> Idle
            energy.max -> Full
            else -> Charging(rate)
        }
    }

    on use(p) { p.open(ChargerScreen(self)) }
}

item Battery {
    stack = 1
    saved synced var charge: Energy in 0..res.battery.capacity = 0
    val room => charge.max - charge
    tooltip => t"charge.battery.tooltip"(charge, charge.max)
}

recipe Battery shaped {
    "RIR"
    "IGI"
    "RIR"
    R = @item:minecraft:redstone
    I = @item:minecraft:iron_ingot
    G = @item:minecraft:gold_ingot
}

player state {
    saved var visits = 0
}

client fn welcome(times: Int) {
    toast(t"charge.welcome"(times))
}

on player.join(p) {
    p.state.visits += 1
    p.client.welcome(p.state.visits)
    if p.state.visits == 1 { p.give(Battery()) }
}

server on server.started {
    Task.every(10s) {
        let lonely = level.entities.near(spawn, 32b).where { it is Monster }.orderBy { it.distance }.take(5)
        for m in lonely { m.glow(5s) }
    }
}

screen ChargerScreen(block: Charger) ui "charger.yui" {
    val energy => block.energy
    val fill => energy / energy.max
    action dump() { block.energy = 0 }
}

command "charge" {
    server fn fill(src: Sender, target: Player = src.player) requires src.isOp {
        target.inventory.each<Battery> { it.charge = it.charge.max }
    }
}

version mc >= 1.21 {
    fn armor(p: Player) -> Int = p.getArmorValue()
} else {
    fn armor(p: Player) -> Int = p.armorValue
}

test "a charger fills a battery" {
    let w = testWorld()
    let c = w.place(Charger, at = origin)
    c.energy = 100
    let b = c.inventory.put(Battery())
    w.tick(1t)
    expect(b.charge == 100 and c.energy == 0)
}
```

```yns
patch LivingEntity {
    add saved var shield: Float = 0
    add is Chargeable

    before hurt(source: DamageSource, amount: Float) -> Bool {
        if shield >= amount {
            shield -= amount
            skip false
        }
    }

    after getMaxHealth() -> Float { result += shield / 2 }

    inside tick() {
        around call aiStep() { if not level.isFrozen { proceed() } }
    }

    inside knockback(strength: Double, x: Double, z: Double) {
        at call Vec3.add first (x) { x * 0.5 }
        constant 0.4 -> 0.2
    }
}

patch MinecraftServer {
    after tickServer(hasTime: BooleanSupplier) uses local start: Long, var local spent: Long #1 {
        if spent > res.limits.slowTick { warn("slow tick: {spent} ns") }
    }
}

patch Player {
    read foodData {
        if hasEffect(@effect:charge:overcharged) { FoodData.full } else { proceed() }
    }
    write experienceLevel(value) {
        if value >= experienceLevel { proceed(value) }
    }
    replace canEat(ignoreHunger: Bool) -> Bool {
        ignoreHunger or foodData.needsFood() or (self as Chargeable).shield > 0
    }
    priority 100
}
```

## 8. Conversions

Implicit only where a conversion is **lossless and always possible**; one
that can fail is `as?` or a member giving an optional; a lossy one is an
explicit call.

| From → to | How | Why |
|---|---|---|
| `Block` → item | `block.item: Item?` | fire, water, portals have no item |
| `Item` → block | `item.block: Block?` | only block items have one |
| `Block` → `BlockState` | implicit (its default state); back: `state.block` | one default, no loss |
| `Item` → `ItemStack` | implicit (one); back: `stack.item` | a stack's count and components are lost going back |
| `Fluid` ↔ `Block` | `fluid.block: Block?`, `block.fluid: Fluid?` | not every fluid has a block |
| content ↔ tag | never; `x in #tag` | a tag is a set |
| `Seconds` → `Ticks` | implicit (×20, exact) | |
| `Ticks` → `Seconds` | explicit `t.seconds` (fractional) | `7t` is 0.35 s |
| `Ticks` ↔ `Duration` | never implicit | game time vs wall time: `/tick rate` and lag make 20 t ≠ 1 s |
| `Blocks` → `Double` | implicit; back explicit | a bare number could be anything |
| `Int` → `Float` / `Double` | implicit (beyond 2⁵³ to `Double`: a warning) | widening |
| `Float` → `Int` | explicit with a rounding: `floor()`, `round()`, `toInt()` (truncates) | coordinates need `floor`: −0.5 is block −1 |
| `Vec3` → `Pos` | explicit `v.blockPos` (floor); `Pos` → `Vec3`: `pos.center` | |
| `Int` → `Int in a..b` | a literal checked at build; an expression: `clamp(…)` or `as?` | a clamp is visible |
| `Str` → `Text` | implicit (literal text) | |
| `Text` → `Str` | `text.resolve()`, `client` only | it needs the client's language |
| a mod's units | declared conversions only (`unit FE = Energy * 1`); different quantities never mix | |
| `Entity` → subtype | `as?` → optional; `is` in `if` / `match` narrows | a downcast can fail |
| another mod's content | `@item:create:wrench` is `Item?` unless that mod is required | it may be absent |

Edge cases: a stack held after it's used up is an empty stack (as in the
game), not `none`; an entity held in a `saved` field reads as `none` once
removed or unloaded (its type is always `Entity?` there); content missing
on a targeted version is a diagnostic for that version, used only inside
`version` blocks.

## 9. Generics

One constraint form: `T: Base` (a class or interface).

```ys
fn heaviest<T: Entity>(list: List<T>) -> T? = list.maxBy { it.mass }
```

`and` / `or` / `not` and `&&` / `||` / `!` are both kept.
