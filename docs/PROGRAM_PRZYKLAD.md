# Przykładowy program w Oli--

To jest mały program testowy dla wysokopoziomowego kompilatora `olic`.
Kompilator istnieje; `oli1` z katalogu `genesis/build` jest natomiast
kompilatorem podzbioru oli-core używanym do budowania łańcucha bootstrapu.

Na podstawie dokumentacji z [LANGUAGE.md](LANGUAGE.md), program ma ten kształt:

```oli
module hello
import std.os

proc start -> s32
    entry
    permit os.syscall
    msg := "Hello from Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

Zapisany w pliku `hello.oli`.

## Kompilacja

Z katalogu repozytorium zbuduj łańcuch raz:

```bash
./genesis/test.sh
```

Następnie dodaj wrapper do `PATH` i skompiluj przykład z jego katalogu:

```bash
export PATH="$HOME/NOWY/bin:$PATH"
cd /home/oliwier/NOWY/docs/projekt_OLI--
olic hello.oli
./hello
```

Program wypisuje `Hello from Oli--`. `olic` korzysta z bibliotek z `lib/`,
więc importy działają również przy wywołaniu z dowolnego katalogu po
skonfigurowaniu wrappera. Diagnostyka:

```bash
olic --check hello.oli
olic --show-opt hello.oli
```

Wynik jest małym statycznym ELF-em bez libc i runtime'u. To działający
przykład kompilacji, nie oznacza jednak ukończenia całego roadmapu: backend,
stdlib, tryb freestanding, kernel i kolejne wersje języka są nadal rozwijane.
Aktualny stan i completion gates opisuje [PROJECT_STATUS.md](PROJECT_STATUS.md).
