# Oli-- po polsku — podręcznik pisania programów

Praktyczny przewodnik po składni, typach, zmiennych, tablicach, procedurach,
wejściu i wyjściu, pamięci, bibliotekach oraz narzędziach Oli--.

Oli-- jest językiem systemowym. Pamięć, zakresy, błędy, uprawnienia i koszt
operacji są jawne. Pliki mają rozszerzenie `.oli`, kompilator nazywa się
`olic`, a bloki zamykają się słowem `end`.

## 1. Pierwszy program

```oli
module hello
import std.os

proc start -> s32
    entry
    permit os.syscall
    text := "Witaj w Oli--!\n"
    os.syscall(os.WRITE, 1, text.addr, text.len)
    ret 0
end
```

Uruchomienie z katalogu głównego projektu:

```sh
sh genesis/test.sh
genesis/build/olic < hello.oli > hello
chmod +x hello
./hello
```

Krócej, skryptem `bin/olic` (po jednym uruchomieniu `sh genesis/test.sh`,
które buduje kompilator):

```sh
bin/olic hello.oli && ./hello
```

`entry` oznacza procedurę startową, a jej wynik `s32` staje się kodem wyjścia.
Oli-- nie wymaga `main`. `os.syscall` wykonuje bezpośrednio syscall Linuksa,
bez libc. Gotowy przykład znajduje się w [`examples/hello.oli`](../examples/hello.oli).

## 2. Składnia

Wcięcia są tylko stylem. Instrukcje zwykle zajmują osobne wiersze, a bloki
kończą się przez `end`.

```oli
-- komentarz do końca wiersza
--- komentarz dokumentacyjny

if value > 0
    ret value
else
    ret 0
end
```

Nazwy rozróżniają wielkość liter. Ścieżki nazw używają kropki: `os.WRITE`,
`hdr.length`, `core.mem`.

```oli
42  1_000_000  64K  0xff  0b1010
'A'  "tekst\n"  true  false  none
```

Najważniejsze operatory to `+ - * / %`, porównania `== != < <= > >=`, bitowe
`& | ^ ~ << >>`, logiczne `and or not` oraz zakres `..`.

```oli
x := a + b          -- binding: nazwa wartości, bez własnego adresu
y : u32 <- 0        -- miejsce typu u32, od razu zainicjalizowane
y <- y + 1          -- zapis do istniejącego miejsca
```

`:=` tworzy niemutowalne powiązanie. `:` tworzy miejsce, które można zmieniać
przez `<-`.

## 3. Typy

Typy liczb całkowitych to `u8`, `u16`, `u32`, `u64`, `s8`, `s16`, `s32`, `s64`,
`word` i `uword`. `byte` oznacza bajt, `bool` wartość logiczną, a `never`
procedurę, która nie wraca.

```oli
addr u8       -- surowy adres bajtu
ref Header    -- referencja do jednego obiektu
rw ref Header -- referencja z prawem zapisu
view u8      -- widok: adres i długość
rw view u8   -- widok z prawem zapisu
zone         -- uchwyt regionu pamięci
[16]u8       -- tablica szesnastu bajtów
```

`view` nie kopiuje danych. Przechowuje adres i długość, a `v[i]` sprawdza
zakres. `ref` wskazuje na obiekt. `addr` jest surowym adresem i wymaga
`permit memory.raw` przy dostępie.

Jawne konwersje:

```oli
small := u32(value)          -- konwersja bezstratna
wrapped := u8.wrap(value)   -- obcięcie do szerokości u8
bits := u64.bits(value)     -- reinterpretacja bitów
```

Zwykła arytmetyka zatrzymuje program przy przepełnieniu. `wrap(e)` włącza
zawijanie modulo szerokości typu:

```oli
total <- wrap(total + item)
```

`sat(e)` nasyca wynik do zakresu typu, a `checked(e)` daje `T or Overflow`,
który obsługuje się przez `else` albo `case`. Obie formy działają
(`tests/run/saturate.oli`); istnieją też `T.sat(x)` i `T.checked(x)`.

## 4. Zmienne i tablice

```oli
proc example -> s32
    entry
    name := "Oli--"
    counter : u64 <- 0
    counter <- counter + 1
    first : u8 <- name[0]
    ret s32(first)
end
```

To jest błąd, bo `x` jest niemutowalnym bindingiem:

```oli
x := 1
x <- 2
```

Poprawnie:

```oli
x : u64 <- 1
x <- 2
```

Tablice mają postać `[N]T`, a literały `{ ... }`:

```oli
values : [4]u32 <- { 10, 20, 30, 40 }
```

Tablica w ramce procedury albo statyczna na poziomie modułu jest czytana jak
`rw view T` swoich bajtów: ma `.len`, `[i]` i działa z `each`. Bufory
przekazuje się dalej jako `view T`, zwykle utworzony z tablicy lub strefy:

```oli
head := buffer[0..8]
tail := buffer[8..]
item := buffer[index]
buffer[index] <- 0
```

Tablice w ramce (`tests/run/frames.oli`), tablice statyczne oraz stałe
tablicowe `NAME : [N]T := { … }` w segmencie tylko do odczytu
(`tests/run/rodata.oli`) działają w backendzie.

## 5. Procedury, czyli funkcje

```oli
proc add(a : u64, b : u64) -> u64
    ret a + b
end

proc checksum(data : view u8) -> u32
    result : u32 <- 0
    each item in data
        result <- wrap(result + item)
    end
    ret result
end
```

Wywołanie:

```oli
sum := add(2, 3)
hash := checksum(bytes)
value := add(a: 2, b: 3)
```

Nie wolno mieszać argumentów pozycyjnych i nazwanych. Procedura może mieć
kontrakt uprawnień:

```oli
proc read_byte(p : addr u8) -> u8
    permit memory.raw
    ret [p]
end
```

Domyślna konwencja to `calls sysv`. `calls none` służy kodowi startowemu, a
`calls interrupt` obsłudze przerwań.

## 6. Warunki i pętle

```oli
if n == 0 then ret 0

if n < 0
    ret -1
elif n == 0
    ret 0
else
    ret 1
end
```

```oli
i : u64 <- 0
sum : u64 <- 0
while i < 10
    sum <- sum + i
    i <- i + 1
end
```

`loop` jest pętlą bezwarunkową. Kończy się przez `break`, `ret` albo `fail`;
`continue` zaczyna następną iterację.

```oli
each b in data
    if b == 0 then continue
    total <- wrap(total + b)
end
```

`each` działa po widoku, po tablicy w ramce i po zakresie liczb całkowitych
`each i in 0..n` (`i` od `0`, dopóki jest mniejsze od `n`):

```oli
each i in 0..n
    total <- total + i
end
```

## 7. Struktury pamięci — `layout`

`layout` opisuje dokładny układ pól w pamięci:

```oli
layout Header
    magic  : u32
    length : be u16
    flags  : u16
end
```

Widok bajtów można zinterpretować bez kopiowania:

```oli
hdr := Header.at(buffer)
magic := hdr.magic
hdr.flags <- 1
```

`Name.size` i `Name.align` są stałymi kompilacji. `packed` usuwa zwykłe
dopełnianie, a `align N` wymusza wyrównanie. `be T` i `le T` oznaczają kolejność
bajtów pola. Zbyt krótki lub źle wyrównany widok powoduje trap `bounds` albo
`misaligned`.

## 8. Pamięć i `zone`

`zone` jest jawnym regionem pamięci. Wszystkie obiekty znikają przy `end`:

```oli
proc make_buffer -> s32
    entry
    permit os.syscall
    zone scratch 64K
        packet := scratch.bytes(4096)
        packet[0] <- 0x4f
        packet[1] <- 0x4c
        ret s32.wrap(packet.len)   -- uword -> s32: jawne obcięcie
    end
end
```

```oli
buf := z.bytes(128)       -- rw view u8, wyzerowany
item := z.make(Header)    -- rw ref Header
```

Nie wolno zwrócić widoku ani referencji do zakończonej strefy. `zone` nie jest
garbage collectorem i nie ma pojedynczego `free` dla każdego obiektu.

## 9. Błędy jako wartości

Procedura może zwrócić `T or E`. `ret` oznacza sukces, a `fail` błąd:

```oli
proc first(data : view u8) -> u8 or none
    if data.len == 0 then fail
    ret data[0]
end
```

Wartość błędną trzeba obsłużyć natychmiast:

```oli
x := first(data) else ret 1       -- przy błędzie zwróć 1
x := first(data) else fail        -- przekaż błąd dalej
x := first(data) else 0           -- wartość domyślna
```

Pełna obsługa używa `case`:

```oli
case parse_header(buffer)
when ok header
    use(header)
when fail too_short
    ret 1
when fail bad_magic { found }
    ret 2
end
```

Są to wartości, nie ukryte wyjątki. Kompilator sprawdza, czy każda ścieżka
błędu jest obsłużona.

## 10. Wejście i wyjście

Obecnie I/O wykonuje się przez `os.syscall`; nie ma jeszcze wbudowanego
runtime'u z funkcjami `print()` i `input()`.

```oli
proc write_text(fd : u64, text : view u8) -> u64
    permit os.syscall
    written := os.syscall(os.WRITE, fd, text.addr, text.len)
    ret u64.bits(written)   -- s64 -> u64: te same bity
end
```

Deskryptory `0`, `1`, `2` oznaczają standardowe wejście, wyjście i wyjście
błędów. Odczyt:

```oli
proc read_stdin(buffer : rw view u8) -> s64
    permit os.syscall
    ret os.syscall(os.READ, 0, buffer.addr, buffer.len)
end
```

Prosty echo:

```oli
proc echo_once -> s32
    entry
    permit os.syscall
    zone input 4K
        buffer := input.bytes(4K)
        n := os.syscall(os.READ, 0, buffer.addr, buffer.len)
        if n <= 0 then ret 1
        os.syscall(os.WRITE, 1, buffer.addr, n)
    end
    ret 0
end
```

Napisy są widokami bajtów bez automatycznego końcowego bajtu `0`.

## 11. Moduły i biblioteki

```oli
module app
import core.mem
import std.os
```

Biblioteki są w `lib/`: między innymi `core.oli`, `core/mem.oli`, `core/x64.oli`,
`core/a64.oli` i `std/os.oli`. Publiczne deklaracje oznacza się `pub`:

```oli
module math

pub proc twice(x : u64) -> u64
    ret x * 2
end
```

Stałe modułowe:

```oli
PAGE : uword := 4096
```

Importy rozwiązują się w katalogu `lib/`, a procedury i stałe biblioteki
wywołuje się przez `mod.proc(...)` i `mod.CONST`. Dane statyczne
(`NAME : T <- expr` na poziomie modułu, w `.data` albo `.bss`), stałe
agregatowe (w `.rodata`) i pliki obiektowe (`-- output: object`,
`extern proc`, `export`) działają; program w Oli-- linkuje się z C w obie
strony (`tests/c/`). Biblioteka standardowa jest jeszcze mała: `std.os`,
`core`, `core.mem`, `core.frames`, `core.x64`, `core.x64.paging`,
`core.x64.sched`, `core.a64` i `std.threads` (sekcja 25).

## 12. Uprawnienia i kod sprzętowy

| Uprawnienie | Dostęp |
|---|---|
| `os.syscall` | wejście do systemu operacyjnego |
| `memory.raw` | surowy adres i `[p]` |
| `cpu.asm` | blok `machine x64` |
| `memory.mmio` | pamięć urządzeń |
| `io.port` | porty sprzętowe |

```oli
proc cpu_vendor -> u32
    permit cpu.asm
    value : u32
    machine x64
        in eax <- 0
        cpuid
        out ebx -> value
        clobber ecx, edx
    end
    ret value
end
```

Blok musi deklarować wejścia (`in`), wyjścia (`out`) i rejestry niszczone przez
instrukcje (`clobber`). Kompilator asembluje go własnym enkoderem; linia,
której nie zna, kończy się `E0900`. Do rejestrów sterujących, `gdt`/`idt`,
`msr` i `cpu.halt()` służy uprawnienie `cpu.control` albo `cpu.halt`, a do
procedur obsługi przerwań `cpu.interrupt`.

## 13. Komendy

Pełna weryfikacja bootstrapa:

```sh
sh genesis/test.sh
```

Kompilacja programu:

```sh
genesis/build/olic < program.oli > program
chmod +x program
./program
```

Podgląd etapów kompilacji:

```sh
genesis/build/show_tokens < program.oli
genesis/build/show_ast    < program.oli
genesis/build/show_sema   < program.oli
genesis/build/show_oir    < program.oli
```

Pomoc narzędzia lokalnego:

```sh
bin/olic --help
```

Skrypt `bin/olic` przyjmuje `PLIK.oli [-o WYJŚCIE]` oraz opcje `--check`,
`--show-tokens`, `--show-ast`, `--show-sema`, `--show-oir`, `--show-ssa`,
`--show-opt`, `--explain` i `--show-asm`. Tryb freestanding, profil celu i
plik obiektowy wybiera się pragmami na początku pliku (`-- target:
freestanding`, `-- profile: PLIK`, `-- output: object`), nie opcją. `--lib DIR`
jest planowane; importy rozwiązują się w `lib/` katalogu głównego. Element
poprawny składniowo, ale bez backendu kończy się błędem `E0900`.

Kernel z przykładów uruchamia pod QEMU skrypt `tools/run-kernel.sh`.

## 14. Diagnostyka

| Kod | Znaczenie |
|---|---|
| `E0014` | brak `end` |
| `E0110` | zapis do niemutowalnego bindingu |
| `E0200` | niezgodność typów |
| `E0202` | konwersja może tracić dane |
| `E0220` | odczyt niezainicjalizowanego miejsca |
| `E0230` | brak `ret` |
| `E0310` | nieobsłużony wynik `T or E` |
| `E0401` | brak wymaganego `permit` |
| `E0900` | funkcja nie ma jeszcze implementacji backendu |

Przepełnienie powoduje trap `overflow` i kod wyjścia 134. Dostęp poza widokiem
powoduje `bounds`, a przekroczenie strefy `zone_exhausted`.

## 15. Pełny przykład

```oli
module sum
import std.os

proc checksum(data : view u8) -> u32
    total : u32 <- 0
    each item in data
        total <- wrap(total + item)
    end
    ret total
end

proc start -> s32
    entry
    permit os.syscall
    zone scratch 4K
        data := scratch.bytes(4)
        data[0] <- 1
        data[1] <- 2
        data[2] <- 3
        data[3] <- 4
        result := checksum(data)
        message := "Suma zostala policzona\n"
        os.syscall(os.WRITE, 1, message.addr, message.len)
        if result != 10 then ret 1
    end
    ret 0
end
```

Widać tu podstawowy styl Oli--: bufor ma jawny region, widok przekazuje adres i
długość, procedura ma określony wynik, a I/O wymaga deklaracji uprawnienia.

## 16. Dalsza dokumentacja

- [`docs/LANGUAGE.md`](LANGUAGE.md) — pełna referencja języka i status funkcji.
- [`docs/COMMANDS.md`](COMMANDS.md) — konstrukcje, koszt i status implementacji.
- [`spec/OLI_SYNTAX_V0.md`](../spec/OLI_SYNTAX_V0.md) — formalna gramatyka.
- [`spec/OLI_SEMANTICS_V0.md`](../spec/OLI_SEMANTICS_V0.md) — semantyka.
- [`spec/OLI_MEMORY_V0.md`](../spec/OLI_MEMORY_V0.md) — model pamięci.
- [`examples/packet_demo.oli`](../examples/packet_demo.oli) — większy przykład.

Status opisany tutaj odnosi się do aktualnego repozytorium. Funkcja oznaczona
jako rozwijana może być już rozpoznawana przez parser, ale nie musi jeszcze
tworzyć wykonywalnego kodu przez `olic`.

## 17. Ściąga składni

### Deklaracje

```oli
module nazwa.modulu
import inny.modul
import inny.modul as alias

STAŁA : u64 := 4096

layout Packet
    field : u32
end

choice Error
    empty
    invalid { code : u32 }
end

proc nazwa(argument : u32) -> u64
    ret u64(argument)
end
```

### Instrukcje

```oli
x := wyrażenie
x : u32
x : u32 <- 0
x <- wyrażenie
v[i] <- wartość

if warunek then ret 0
if warunek
    ...
elif inny_warunek
    ...
else
    ...
end

while warunek
    ...
end

each element in widok
    ...
end

loop
    ...
    break
end
```

### Wyrażenia pamięci

```oli
v.len                 -- długość widoku
v.addr                -- adres pierwszego elementu
v[i]                  -- element z kontrolą zakresu
v[a..b]               -- podwidok
v[..b]                -- od początku do b
v[a..]                -- od a do końca
T.size                -- rozmiar layoutu
T.align               -- wyrównanie layoutu
T.at(v)               -- referencja T nad bajtami widoku
z.bytes(n)            -- bufor ze strefy
z.make(T)             -- obiekt ze strefy
```

## 18. Operatory i kolejność działań

Od najsłabiej do najsilniej wiążących:

| Poziom | Operatory |
|---:|---|
| 1 | `else` |
| 2 | `..` |
| 3 | `or` |
| 4 | `and` |
| 5 | `== != < <= > >=` |
| 6 | `\|` |
| 7 | `^` |
| 8 | `&` |
| 9 | `<< >>` |
| 10 | `+ -` |
| 11 | `* / %` |
| 12 | `not`, znak `-`, `~`, `addr`, `ref` |
| 13 | wywołanie `()`, pole `.`, indeks `[]` |

Gdy wyrażenie może być niejasne, użyj nawiasów:

```oli
result <- (a + b) * c
ok := (left < right) and (count != 0)
```

Porównania nie mogą być łańcuszkiem. Zapis `a < b < c` należy zastąpić przez
`(a < b) and (b < c)`.

## 19. `choice`, warianty i dopasowanie

`choice` opisuje wartość, która może mieć jeden z nazwanych wariantów. Wariant
może przechowywać pola:

```oli
choice ParseError
    too_short
    bad_magic { found : u32 }
end
```

Procedura może zwrócić taki błąd:

```oli
proc parse(data : view u8) -> u32 or ParseError
    if data.len < 4 then fail too_short
    if data[0] != 0x4f then fail bad_magic { found: u32(data[0]) }
    ret u32(data[0])
end
```

Obsługa wariantów używa `case`:

```oli
case parse(data)
when ok value
    use(value)
when fail too_short
    ret 1
when fail bad_magic { found }
    ret 2
end
```

Każdy wariant powinien być obsłużony albo należy dodać końcowe `else`
(brak wariantu to `E0311`). Wartości `choice` działają w backendzie: te, które
mieszczą się w ośmiu bajtach, podróżują w jednym słowie, szersze przez swój
adres (`tests/run/choice.oli`, `tests/run/records.oli`). `case` działa też nad
`bool` i nad liczbą całkowitą.

## 20. Freestanding i kernel

Program hosted korzysta z systemu operacyjnego i zaczyna od procedury z
`entry -> s32`. Program freestanding działa bez libc i bez zwykłego runtime'u.
Jego punkt wejścia zwykle nie wraca:

```oli
-- target: freestanding
module boot

boot_stack : [16K]u8
    section ".bss.boot"
    align 16

proc start -> never
    entry
    calls none
    section ".text.boot"
    permit cpu.asm
    machine x64
        lea rsp, [boot_stack + 16K]
        xor ebp, ebp
        call boot.main
        hlt
    end
end

proc main -> never
    permit cpu.halt, memory.raw
    zone heap 64K at 0x100000
        -- pamięć pod adresem dostarczonym przez bootloader
        buf := heap.bytes(16)
        buf[0] <- 1
    end
    loop
        cpu.halt()
    end
end
```

Pragma `-- target: freestanding` w pierwszym wierszu wybiera tryb. Wejście ma
`calls none` (bez ramki) i samo ustawia stos w bloku `machine x64`, a `section
".text.boot"` stawia je na początku kodu. W trybie freestanding pamięć strefy
musi pochodzić z jawnego adresu (`at`) albo od rodzica (`from`); `cpu.halt()`
wymaga uprawnienia `cpu.halt`. Procedura oznaczona `traps` odbiera każdy trap
jako `core.TrapKind` i `core.Site`.

Kod kernela korzysta z `machine x64`, `memory.raw`, MMIO, portów, layoutów
struktur sprzętowych i własnego punktu wejścia. Przykłady znajdują się w
[`examples/kernel.oli`](../examples/kernel.oli) oraz w
[`docs/FREESTANDING.md`](FREESTANDING.md) i
[`docs/KERNEL_PROGRAMMING.md`](KERNEL_PROGRAMMING.md).

## 21. Jak działa wywołanie procedury

Domyślna konwencja Oli-- to SysV x86-64. Argumenty całkowitoliczbowe trafiają
najpierw do rejestrów, a następne na stos. Wynik jest zwracany w rejestrze.
Widok `view T` zajmuje dwa słowa: adres i długość.

```oli
proc add(a : u64, b : u64) -> u64
    ret a + b
end
```

Wartość fallible `T or E` ma znacznik sukcesu/błędu oraz payload. Dzięki temu
`fail` nie wymaga wyjątków ani ukrytego stosu obsługi.

Szczegóły binarnego interfejsu znajdują się w
[`docs/ABI.md`](ABI.md). Są przydatne przy łączeniu z kodem C, pisaniu
bootloadera i analizie wygenerowanego ELF-a.

## 22. Koszt operacji

Oli-- stara się, aby koszt był widoczny w kodzie:

| Operacja | Typowy koszt |
|---|---|
| binding `:=` | rejestr lub proste przeniesienie |
| miejsce `: T` | miejsce w ramce stosu |
| `v[i]` | kontrola zakresu i odczyt |
| `z.bytes(n)` | alokacja bump-pointer w strefie |
| wywołanie procedury | `CALL` i konwencja ABI |
| `os.syscall(...)` | wejście do kernela |
| `machine x64` | instrukcje sprzętowe |
| przepełnienie | celowy trap |

Optymalizator może usunąć kontrolę zakresu, jeśli potrafi udowodnić, że indeks
jest poprawny. Nie usuwa kontroli bez takiego dowodu.

## 23. Struktura repozytorium

```text
examples/       programy demonstracyjne
lib/            moduły core i std
compiler/       źródła kompilatora olic w Oli--
tests/          testy parsera, semantyki i uruchamiania
genesis/        bootstrap: hex0 -> hex2 -> asm -> oli1 -> olic
docs/           dokumentacja projektowa
spec/           formalne specyfikacje V0
bin/            skrypt olic (wiersz poleceń kompilatora)
tools/          run-kernel.sh (QEMU) i rozszerzenie VS Code
```

Najlepsze pliki do nauki w kolejności:

1. [`examples/hello.oli`](../examples/hello.oli) — syscall i wyjście.
2. [`examples/loop_sum.oli`](../examples/loop_sum.oli) — miejsca, `while` i procedura z wynikiem.
3. [`examples/fibonacci.oli`](../examples/fibonacci.oli) — rekurencja.
4. [`examples/function_values.oli`](../examples/function_values.oli) — bindingi zachowują wartość między wywołaniami.
5. [`genesis/3-oli1/tests/layout_basic.oli`](../genesis/3-oli1/tests/layout_basic.oli) — layout i zone.
6. [`examples/packet_demo.oli`](../examples/packet_demo.oli) — większy program systemowy.
7. [`examples/kernel.oli`](../examples/kernel.oli) — kod freestanding, kernel dla QEMU.

## 24. Praktyczna kolejność pisania programu

1. Zacznij od `module` i importów.
2. Zdefiniuj `layout` dla danych, które mają stały układ.
3. Napisz małe `proc` z typami parametrów i wyniku.
4. Rozdziel bindingi `:=` od miejsc zapisywalnych `: T <- ...`.
5. Utwórz `zone`, jeśli potrzebujesz bufora lub obiektu.
6. Dodaj `permit` dopiero w procedurze, która rzeczywiście korzysta z danego zasobu.
7. Obsłuż każdą wartość `T or E` przez `else` albo `case`.
8. Skompiluj program i sprawdź komunikaty diagnostyczne.
9. Dopiero potem dodawaj kod sprzętowy, raw memory i optymalizacje.

Minimalny szablon nowego programu:

```oli
module nazwa
import std.os

proc start -> s32
    entry
    permit os.syscall
    -- deklaracje
    -- obliczenia
    -- I/O
    ret 0
end
```

## 25. Najważniejsze moduły biblioteczne

### `std.os`

```oli
import std.os

os.READ
os.WRITE
os.CLOSE
os.MMAP
os.MUNMAP
os.EXIT
os.EXIT_GROUP
os.syscall(...)
```

Stałe syscalli są dobierane do architektury. `std.os.Error` opisuje ujemny
wynik zwrócony przez system operacyjny.

### `core.mem`

```oli
import core.mem

same := mem.equal(left, right)
```

Moduł `core.mem` zawiera operacje na widokach bajtów, między innymi
`mem.equal`. Pozostałe operacje niskopoziomowe mogą być używane przez własne
procedury i bezpośrednie widoki.

### `core`

```oli
import core

proc on_trap(kind : core.TrapKind, site : core.Site) -> never
    ...
end
```

`core` definiuje podstawowe typy używane przez kod freestanding, w tym
`TrapKind`, `Site` i `CpuId`.

### `core.x64` i `core.a64`

Moduły architektury zawierają layouty ramek wyjątków i struktur procesora.
Przykładowo `core.x64.InterruptFrame` służy procedurom obsługi przerwań na
x86-64, a `core.a64.ExceptionFrame` na AArch64.

### `core.x64.paging`

Moduł udostępnia flagi tablic stron oraz funkcje obliczające indeksy poziomów:

```oli
import core.x64.paging

entry := paging.make_entry(target, paging.PRESENT | paging.WRITABLE)
i := paging.pml4_index(address)
```

Ten moduł jest przeznaczony głównie dla kernela i trybu freestanding.

### `std.threads`

Wątki na hostowanym x86-64: `threads.spawn(slot, stos, wejście)` to jedno
wywołanie `clone` ze stosem, który należy do programu, a `threads.join(slot)`
czeka na futexie na słowo, które kernel zeruje po zakończeniu wątku. Wątek
to jego słowo `u32` w jednoelementowym widoku (`tids[i..i + 1]`); wejście to
procedura bez parametrów, której adres bierze się z bloku `machine x64`
(`mov rax, worker; out rax -> e`). Przykład: `tests/run/threads.oli`.
(`thread` jest słowem zarezerwowanym, stąd `std.threads`.)

```oli
import std.threads

tids : [4]u32
stacks : [64K]u8
    align 16

r := threads.spawn(tids[0..1], stacks[..16K], e)
threads.join(tids[0..1])
```

### `core.frames` i `core.x64.sched`

`core.frames` to menedżer pamięci fizycznej: mapa bitowa ramek 4 KiB
(`frames.init`, `frames.load_multiboot`, `frames.alloc`, `frames.alloc_run`,
`frames.release`, `frames.count_free`). `core.x64.sched` to scheduler:
zadania na własnych stosach, `sched.spawn`, `sched.yield`, `sched.exit`,
`sched.ready` i `sched.switch`; kooperacyjny, a wywołany z procedury obsługi
przerwania timera (`sched.preempt`) wywłaszczający. Oba używa
`examples/kernel.oli`.

## 26. Ćwiczenia dla początkujących

### Ćwiczenie 1 — kod wyjścia

Napisz program, który nie wypisuje tekstu, ale kończy się kodem `42`. Użyj
procedury `start -> s32`, `entry` oraz `ret 42`.

### Ćwiczenie 2 — suma

Napisz `proc sum_to(n : u64) -> u64`, która zwraca sumę od `0` do `n`. Użyj
`while`, dwóch miejsc typu `u64` i jawnych zapisów `<-`.

### Ćwiczenie 3 — silnia

Napisz rekurencyjną procedurę `factorial`. Dla `0` zwróć `1`, a dla większej
wartości wywołaj procedurę dla `n - 1`.

### Ćwiczenie 4 — liczenie bajtów

Napisz `count_byte(data : view u8, wanted : u8) -> u64`, która przechodzi po
widoku przez `each` i liczy wystąpienia wybranego bajtu.

### Ćwiczenie 5 — echo

Utwórz `zone` o rozmiarze `4K`, wczytaj dane przez `os.READ`, a następnie
wypisz tyle bajtów, ile zwrócił syscall. Obsłuż wynik mniejszy lub równy zero.

### Ćwiczenie 6 — layout

Zdefiniuj `layout Header` z polami `magic : u32`, `length : u16` i
`flags : u16`. Utwórz bufor w `zone`, uzyskaj `Header.at(buffer)` i wpisz
wartości do wszystkich pól.

### Ćwiczenie 7 — wynik fallible

Napisz procedurę `digit(c : u8) -> u8 or none`. Dla znaków od `'0'` do `'9'`
zwróć wartość cyfry, a dla pozostałych użyj `fail`. Obsłuż wynik przez `case`.

### Ćwiczenie 8 — biblioteka

Utwórz moduł `math`, dodaj publiczną procedurę `double`, zaimportuj go w
programie i użyj jej w `start`.

Gotowe podobne programy można porównać z katalogiem
[`genesis/3-oli1/tests`](../genesis/3-oli1/tests) oraz z przykładami
[`examples`](../examples). Rozwiązania ćwiczeń 2 i 3 w stylu tego podręcznika
to [`examples/loop_sum.oli`](../examples/loop_sum.oli) i
[`examples/fibonacci.oli`](../examples/fibonacci.oli); wszystkie trzy nowe
przykłady (także `function_values.oli`) kompiluje i uruchamia
`genesis/test.sh`.

## 27. Jak testować własny program

Najpierw sprawdź najmniejszy przypadek:

```sh
genesis/build/olic < program.oli > /tmp/program
chmod +x /tmp/program
/tmp/program
echo $?
```

Potem sprawdź analizę bez uruchamiania backendu:

```sh
genesis/build/show_tokens < program.oli
genesis/build/show_ast < program.oli
genesis/build/show_sema < program.oli
```

Jeśli program nie przechodzi, czytaj pierwszy błąd od góry. Kolejne błędy mogą
być tylko skutkiem pierwszego brakującego `end`, złego typu albo nieobsłużonego
`T or E`.

Testuj osobno:

1. pusty bufor,
2. jeden element,
3. pełny bufor,
4. wartość graniczną typu,
5. indeks równy długości,
6. błąd syscalla,
7. każdą gałąź `case`.

Indeks równy `v.len` jest już poza widokiem. Ostatni poprawny indeks to
`v.len - 1`, gdy widok nie jest pusty.

## 28. Dobre praktyki

- Nadawaj typ każdemu miejscu, które ma znaczenie dla pamięci lub ABI.
- Używaj `:=` dla wartości, które nie powinny być zmieniane.
- Używaj `<-` wyłącznie do zapisu w istniejące miejsce.
- Zostawiaj komentarz przy każdym surowym adresie i syscallu.
- Ograniczaj `permit` do najmniejszej procedury, która go potrzebuje.
- Przekazuj bufory jako `view`, zamiast kopiować ich zawartość.
- Dziel większy program na małe procedury z jednym zadaniem.
- Sprawdzaj długość przed `Layout.at` i przed każdym ręcznym przesunięciem.
- Używaj `wrap` tylko wtedy, gdy zawijanie jest zamierzoną częścią algorytmu.
- Nie zwracaj referencji do lokalnego miejsca ani danych ze zniszczonej strefy.
- Trzymaj kod sprzętowy w osobnych procedurach i modułach architektury.

## 29. Co jest gotowe, a co jest rozwijane

Wszystko, co pokazuje ten podręcznik, kompiluje się i działa: bindingi,
miejsca, arytmetyka z trapem przy przepełnieniu oraz `wrap`/`sat`/`checked`,
warunki, `while`, `loop`, `each` po widoku, tablicy i zakresie, procedury i
rekurencja, napisy, widoki, tablice, strefy, layouty (także pola `be`/`le`),
referencje, wyniki fallible, `choice`, dane statyczne i stałe, bloki
`machine x64`, syscalle Linuksa, tryb freestanding z własnym wejściem,
przerwania (`calls interrupt`), MMIO, porty, atomiki, wątki (`std.threads`
na x86-64), `own T`, liczby zmiennoprzecinkowe `f32`/`f64` i wektory
128-bitowe. Kompilator `olic` jest
napisany w Oli-- i kompiluje sam siebie do identycznych bajtów
(`genesis/test.sh`, warstwa 6). Przykładowy kernel `examples/kernel.oli`
uruchamia się pod QEMU (przerwania, stronicowanie, scheduler wywłaszczający
z timera, zadanie w ring 3 rozmawiające z kernelem przez `int 0x80`), a drugi
backend, AArch64, startuje cztery CPU przez PSCI i uruchamia prawie cały korpus
testów pod `qemu-aarch64` i własny kernel pod `qemu-system-aarch64`.

Rozwijane albo planowane: generyki, wątki na AArch64, szersze wektory (AVX), cel Windows
(PE/COFF), biblioteka standardowa poza `std.os` i `core.*`, kolekcje z jawnymi
alokatorami, opcja `--lib`, wiele procesorów na x86-64 (na AArch64 cztery CPU
już startują przez PSCI) oraz IDE. Konstrukcja, która ma składnię, ale nie ma
jeszcze backendu, kończy się `E0900`; to informacja o braku implementacji, nie
o błędzie w programie.

Najbardziej wiarygodnym źródłem bieżącego statusu jest
[`docs/LANGUAGE.md`](LANGUAGE.md) (każda konstrukcja z oznaczeniem, czy
działa) i [`docs/COMMANDS.md`](COMMANDS.md), planu — [`ROADMAP.md`](../ROADMAP.md),
a formalnych reguł — katalog [`spec`](../spec).
