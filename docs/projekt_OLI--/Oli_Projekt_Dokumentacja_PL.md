# Oli-- — dokumentacja projektu i języka

## 1. Cel i założenie projektu

Oli-- to język systemowy i niskopoziomowy, zaprojektowany tak, aby był przejrzysty dla programisty, a jednocześnie bezpośrednio zmapowany na model maszyny i pamięci. Jego główne założenia to:

- brak ukrytej alokacji i ukrytego przepływu sterowania,
- jawna pamięć i regiony pamięci,
- brak ukrytych wyjątków i mechanizmów automatycznego odzyskiwania pamięci,
- pełna kontrola nad pojemnością, cennymi zasobami, uprawnieniami i kosztami operacji,
- kompilator jako narzędzie, które opisuje i tłumaczy program, a nie ukrywa detale.

Język został zaprojektowany jako część autonomicznego łańcucha narzędziowego, bez użycia Rust, C, C++, LLVM ani external assembler/linker. To jest bardzo ważne: jest to projekt z zamysłem samowystarczalności i własnej bootstrapowej architektury.

Warto od razu rozróżnić trzy poziomy:

1. Język użytkownika — wysokopoziomowa składnia systemowa, która ma być czytelna i deterministyczna.
2. Model pamięci — regiony, widoki, adresy, ref-y, zone-y i prawidłowe zasady ich użycia.
3. Kompilator i backend — parser, semantic analysis, OIR, SSA, obniżanie do x86-64 i zapis ELF.

---

## 2. Główna filozofia języka

Dokumenty projektu opisują Oli-- jako język z następującymi zasadami:

- Wszystko ma swoją cenę: operacje, wywołania, kontrola zakresów i dostęp do pamięci są widoczne w kodzie.
- Błędy są wartościami, a nie ukrytymi wyjątkami.
- Pamięć jest jawna i regionalna.
- Uprawnienia są przyznawane, nie zakładane.
- Każda instrukcja jest zapisana w prostym stylu: jedna instrukcja na linię, bloki zamykane przez `end`.

To oznacza, że kod Oli-- jest z natury niskopoziomowy, ale nadal ma czytelną składnię i czyste semantycznie reguły.

---

## 3. Podstawowa składnia

### 3.1 Moduł i import

```oli
module hello
import std.os
```

- `module` definiuje jednostkę kompilacji.
- `import` wczytuje inne moduły.
- Moduły i nazwy są rozwiązywane przez kompilator w środowisku bibliotecznym.

### 3.2 Procedura

```oli
proc start -> s32
    entry
    permit os.syscall
    msg := "Hello from Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

Najważniejsze elementy:

- `proc` deklaruje procedurę,
- `entry` wskazuje punkt wejścia programu,
- `permit` przyznaje uprawnienie do dostępu do specjalnych zasobów,
- `:=` wiąże nazwę z wartością (bind),
- `ret` zwraca wynik,
- `end` zamyka blok procedury.

### 3.3 Komentarze

```oli
-- zwykły komentarz
--- komentarz dokumentacyjny
```

- `--` komentarz zwykły,
- `---` komentarz dokumentacyjny, który jest przechowywany w AST i może być powiązany z deklaracją.

---

## 4. Pamięć i model regionów

W Oli-- pamięć nie jest „po prostu wskaźnikiem”. Pamięć należy do regionów:

- region statyczny,
- region lokalny / ramek,
- region strefy (`zone`),
- region surowy (`raw addr`),
- region widoku (`view`).

### 4.1 Strefy (`zone`)

`zone` przypomina region alokacji na stosie lub kolejce bump-pointer:

```oli
zone scratch 64K
    pkt := scratch.bytes(4096)
    hdr := scratch.make(Header)
end
```

- pamięć jest przydzielana jawnie,
- cała strefa znika na końcu bloku,
- brak automatycznego GC,
- pozostała pamięć jest zwalniana w jednym kroku.

### 4.2 Widoki (`view`)

Widok to okno do istniejącej pamięci. To nie jest kopia:

```oli
head := packet[0..20]
byte := packet[i]
len  := packet.len
```

Widok ma:

- adres (`addr`),
- długość (`len`),
- elementy są sprawdzane w zakresie,
- dostęp poza zakresem jest traktowany jako błąd / trap.

### 4.3 Adresy i referencje

Oli-- ma odrębne pojęcia:

- `addr T` — surowy adres do pamięci,
- `ref T` — bezpieczny odwołnik do obiektu,
- `view T` — zakres widoku,
- `raw` — niebezpieczny dostęp do adresu z odpowiednim uprawnieniem.

To jest kluczowe: język bardzo wyraźnie oddziela bezpieczny dostęp od sztucznej, niskopoziomowej manipulacji pamięcią.

---

## 5. Struktury danych i układy pamięci

Język umożliwia deklarowanie układów pamięci (layouts):

```oli
layout Header
    len : u32
    flags : u8
end
```

Możliwe są też typy z wyraźnym porządkiem bajtów typu `be` / `le` oraz kontrola align / packed. Daje to językowi możliwość pracy z formatami sieciowymi, nagłówkami, strukturami systemowymi i formatami plików.

---

## 6. Kontrola przepływu

Oli-- wspiera standardowe konstrukcje:

```oli
if a > b then ret a
elif n == 0
    tag <- 'z'
else
    tag <- 'p'
end

while i < n
    ...
end

loop
    ...
    break
end
```

- `if / elif / else`
- `while`
- `loop`
- `break`
- `continue`
- `each` iteracja po widoku lub zakresie

To są instrukcje z wyraźnym, deterministycznym zachowaniem.

---

## 7. Uprawnienia i bezpieczeństwo

W Oli-- nie ma „wielkich możliwości bez pytań”. Odpowiednie operacje żądają specjalnych uprawnień:

- `permit os.syscall`
- `permit memory.raw`
- `permit cpu.asm`
- `permit memory.mmio`
- `permit io.port`

To ma dwa znaczenia:

1. ogranicza dostęp do niebezpiecznych mechanizmów,
2. sprawia, że kompilator może weryfikować, czy kod rzeczywiście ma prawo do wykonania tej operacji.

---

## 8. Komendy i narzędzia

Dokumenty repozytorium opisują następujące główne narzędzia i scenariusze działania.

### 8.1 Narzędzia bazowe / genesis

| Polecenie | Znaczenie | Status |
|---|---|---|
| `sh genesis/test.sh` | pełna weryfikacja wszystkich warstw bootstrapa | działa |
| `genesis/build/oli1.bin` | kompilator oli-core | działa w warstwie genesis |
| `genesis/build/show_tokens` | wypisuje tokeny | działa |
| `genesis/build/show_ast` | wypisuje AST | działa |
| `genesis/build/show_sema` | wypisuje semantyczny graf programu | działa |
| `genesis/build/show_oir` | wypisuje OIR | działa |
| `genesis/build/show_ssa` | wypisuje SSA | działa |
| `genesis/build/show_opt` | wypisuje OIR po optymalizacji | działa |
| `genesis/build/asm.bin` | assembler `machine x64` | działa |

### 8.2 Komendy docelowe `olic`

Planowane polecenia docelowego kompilatora:

| Polecenie | Znaczenie |
|---|---|
| `olic file.oli` | kompilacja do ELF64 |
| `olic --show-tokens` | wypisanie tokenów |
| `olic --show-ast` | wypisanie AST |
| `olic --show-sema` | wypisanie grafu semantycznego |
| `olic --show-oir` | wypisanie OIR |
| `olic --show-ssa` | wypisanie SSA |
| `olic --show-oir=opt` | wypisanie OIR po optymalizacji |
| `olic --check` | analiza bez generacji kodu |
| `olic --freestanding` | tryb bez systemu operacyjnego |
| `olic --lib DIR` | ścieżka do bibliotek |
| `olic --explain` | wyjaśnienie kosztu i struktury procedury |

### 8.3 Wartość i sens każdej komendy

Każde polecenie ma mieć dwie właściwości:

- czy kompilator rozumie język i generuje kod,
- czy potrafi przedstawić strukturę programu w czytelnej formie dla programisty.

To odróżnia Oli-- od projektów, w których kompilator jest „czarną skrzynką”. Tutaj kompilator ma być wyjaśniający i transparentny.

---

## 9. Architektura kompilatora

### 9.1 Warstwy genesis

Repozytorium opisuje pipeline bootstrapa:

| Warstwa | Co buduje | Status |
|---|---|---|
| 0 | `hex0.bin` | zrobione |
| 1 | `hex2.bin` | zrobione |
| 2 | assembler `machine x64` | zrobione |
| 3 | oli-core compiler (`oli1`) | zrobione |
| 4 | front-end (`compiler/`) | zrobione jako parser + sema |

### 9.2 Front-end

Front-end składa się z modułów:

- lexer,
- AST,
- parser,
- loader/imports,
- semantyka,
- typowanie,
- kontrola przepływu i regionów,
- diagnostyka.

### 9.3 Middle-end i backend

Po semantyce pojawiają się:

- OIR,
- CFG,
- SSA,
- optymalizacje,
- lowering do x86-64,
- zapis ELF64.

Ten pipeline ma w zamyśle tworzyć natywny plik wykonywalny bez zewnętrznego linker a i libc.

---

## 10. Freestanding i kernel programming

Dwa duże obszary pracy projektu to:

1. programy działające bez systemu operacyjnego,
2. programy typu kernel / driver / boot code.

### 10.1 Freestanding

Freestanding to model, w którym program nie zakłada istniejącego OS.

- brak `os.syscall`,
- wejście to własna procedura `entry`,
- brak `std`,
- własny stack,
- brak libc,
- dokładne kontrolowanie sekcji i adresów.

### 10.2 Kernel

Język przewiduje obsługę:

- MMIO,
- portów,
- CPU intrinsics,
- wyjątków,
- `calls interrupt`,
- `machine` blocks do bezpośredniej pracy na instrukcjach procesora.

To czyni z Oli-- język odwołujący się do architektury sprzętowej i systemów operacyjnych bez ukrywania warstwy niskopoziomowej.

---

## 11. Status realizacji

Według dokumentacji projekt znajduje się na różnych etapach:

- część podstawowych konstrukcji działa,
- część jest analizowana i zaakceptowana semantycznie,
- część jest zarezerwowana dla wyższych wersji języka,
- część jest dopiero zaplanowana.

Najważniejsze jest to, że dokumenty jasno rozróżniają:

- `runs` — działa i ma testy,
- `analysed` — parser / semantyka rozumie, ale backend nie emituje kodu,
- `reserved` — struktura jest zastrzeżona dla późniejszych wersji,
- `planned` — zaplanowane, ale niezaimplementowane.

---

## 12. Przykładowy program Oli--

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

To jest najprostszy wzorzec programu zgodny z dokumentacją języka.

---

## 13. Dlaczego to ważne

Oli-- nie jest zwykłym „językiem systemowym”. To projekt całkowicie samodzielny, w którym:

- kompilator jest częścią języka,
- narzędzia są zaprojektowane od zera,
- pamięć i uprawnienia są jawne,
- architektura sprzętowa jest obsługiwana bez ukrywania szczegółów,
- programista jest odpowiedzialny za sposób działania programu.

To sprawia, że jest to narzędzie dla ludzi, którzy chcą rozumieć nie tylko to, co robi kod, ale też jak to jest wykonywane na poziomie maszyny.

---

## 14. Podsumowanie

Oli-- łączy w sobie trzy rzeczy:

1. język low-level z czytelną składnią,
2. model pamięci i uprawnień z kontrolą bezpieczeństwa,
3. kompilator z transparentnym, emisyjnym pipeline do ELF/x86-64.

Jego celem nie jest „zrobić wszystko automatycznie”. Celem jest dać programiście pełną świadomość, jak program działa, gdzie znajdują się dane, jakie są koszty operacji i jakie obowiązują reguły bezpieczeństwa.

To jest wersja Polska dokumentacji projektu Oli--.
