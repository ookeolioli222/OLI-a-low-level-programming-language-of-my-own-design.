# Oli--: ocena jako język niskopoziomowy

## Werdykt

Oli-- jest rzeczywistym, natywnie kompilowanym projektem języka systemowego dla
x86-64. Nie jest interpreterem, maszyną wirtualną ani generatorem kodu C.
Kompilator generuje instrukcje x86-64, zapisuje własny ELF64 i pozwala uruchomić
program bez libc, runtime'u i zewnętrznego linkera. W repozytorium jest też
freestandingowy przykład kernela uruchamiany pod QEMU.

Jednocześnie nie jest jeszcze zamiennikiem C. Najtrafniej opisać go jako
ambitny, częściowo samohostujący prototyp języka systemowego: fundamenty są
realne, ale przenośność, biblioteka, stabilność narzędzi i kompletność języka
są dużo mniejsze niż w C.

Stan opisany niżej dotyczy drzewa z 25 września 2026 r. (etap 52). Pełne
`sh genesis/test.sh` przechodzi wszystkie sześć warstw, łącznie z punktem
stałym samohostowania: `olic` kompiluje własne źródło do 1 635 448 bajtów, a
tak zbudowany kompilator odtwarza te same bajty (stage2 == stage3).

## Co jest prawdziwą implementacją

- Frontend obejmuje lexer, parser, AST, analizę typów, regionów, przepływu,
  capabilities i wartości liniowych `own`.
- OIR, CFG, dominatory, SSA, phi nodes, weryfikator i optymalizacje działają na
  etapie kompilacji. Nie są częścią runtime'u programu.
- Backend emituje kod x86-64 i własny ELF64. Wynik jest wykonywany przez
  procesor jako natywny kod.
- Przepełnienie, dzielenie przez zero, bounds check, wyrównanie i wyczerpanie
  zone mogą powodować wygenerowany trap. To są normalne instrukcje i ścieżki
  kodu, a nie wyjątki z wirtualnej maszyny.
- `machine x64`, port I/O, MMIO, atomiki, bariery, rejestry sterujące,
  interrupt handlers i sekcje ELF schodzą do instrukcji albo danych obrazu.
  Kompilator kontroluje ich składnię i wymagane `permit`.
- `calls none`, własny stos, profile obrazu, Multiboot, przejście do long mode,
  IDT, PIC, PIT i prosty allocator ramek pokazują, że język potrafi wyjść poza
  zwykły program użytkownika. `examples/kernel.oli` jest demonstracją tej
  ścieżki.

## Co jest abstrakcją kompilatora albo symulacją

Słowo „symulowane” nie oznacza tutaj, że program działa w emulatorze. Oznacza,
że dana rzecz jest pojęciem języka, które po kompilacji zostaje zastąpione
konkretnym układem pamięci i instrukcjami.

| Pojęcie Oli-- | Co faktycznie powstaje |
|---|---|
| `zone` | bump allocator: `base`, `cursor`, `limit`; w trybie hosted zwykle `mmap`/`munmap`, w kernelu pamięć dostarczona przez program |
| `view T` | para słów `(adres, długość)`; dostęp indeksowany dostaje porównanie zakresu |
| `ref T` | adres obiektu; nie jest magicznym wskaźnikiem z osobnym runtime'em |
| `own T` | kontrola liniowego użycia na etapie semantyki; zwykle zwykła wartość o reprezentacji uchwytu |
| `permit` | deklaracja sprawdzana przez kompilator; nie jest sandboxem ani ochroną procesora |
| regiony i lifetimes | informacje używane do odrzucania uciekających widoków/refów; nie ma garbage collectora |
| `T or E` / `case` | jawny tag i payload oraz skoki; nie są wyjątkami |
| `OIR`, SSA, proof of check removal | reprezentacja i dowody kompilacji, usuwane przed uruchomieniem |
| `core.trap` | wygenerowana procedura/ścieżka błędu zapisująca komunikat albo przekazująca trap do kodu freestandingowego |
| `mmio`, `port`, `cpu.*` | typowane wejście do prawdziwych instrukcji volatile, `in/out`, `cli/sti`, `hlt`, `mov cr*` itd. |
| QEMU | emulacja CPU, pamięci i urządzeń w teście kernela; nie zastępuje testu na prawdziwym sprzęcie |

Najważniejsza granica brzmi: capability i regiony pomagają wykrywać błędy i
porządkować kod, ale nie tworzą izolacji bezpieczeństwa. Program z
`memory.raw`, odpowiednimi adresami i uprawnieniami nadal może uszkodzić pamięć
albo urządzenie. Ochronę daje dopiero MMU, poziom uprzywilejowania i system
operacyjny.

## Oli-- a C

| Obszar | Oli-- | C |
|---|---|---|
| Model wykonania | Natywny x86-64 ELF, własny backend | Natywny kod przez dojrzałe kompilatory i wiele backendów |
| Pamięć | Jawne `zone`, `view`, `ref`, regiony i wybrane kontrole | Wskaźniki, tablice i ręczne zarządzanie; wiele zachowań zależy od UB |
| Przepełnienie | Zwykła arytmetyka może trapować; `wrap`, `sat`, `checked` są jawne | Zależy od signed/unsigned i kontekstu; signed overflow jest UB |
| Dostęp sprzętowy | `machine`, MMIO, porty, rejestry i capabilities jako element języka | Zwykle rozszerzenia kompilatora, inline asm, volatile i nagłówki platformy |
| Błędy | `T or E`, `fail`, `case`, bez ukrytych wyjątków | Najczęściej kody zwrotne, `errno`, `abort` albo własne konwencje |
| ABI i biblioteki | Głównie własny x86-64/SysV; małe `core`/`std`; FFI jest ograniczone | Wieloletni ekosystem, libc, nagłówki, debugery, linkery i biblioteki |
| Przenośność | x86-64 i AArch64 (ELF); inne architektury i PE/COFF pozostają pracą | Linux, Windows, macOS, embedded i wiele architektur |
| Dojrzałość | Prototyp badawczy z własnym łańcuchem bootstrapu | Stabilny standard i produkcyjne narzędzia |
| Optymalizacja | Własne OIR i optymalizacje, ale konserwatywny backend i mały zakres | Dziesięciolecia optymalizacji GCC/Clang/LLVM/ICC |
| Bezpieczeństwo | Część błędów odrzucana statycznie, raw access pozostaje odpowiedzialnością autora | Większa swoboda, ale dużo łatwiej o UB i błędy lifetime/aliasing |

Oli-- ma przewagę jako eksperyment projektowy: semantyka pamięci i koszt
operacji są jawne, a ścieżka od źródła do bajtów jest audytowalna. C wygrywa
praktycznie: bibliotekami, narzędziami, portowalnością, debuggingiem,
optymalizacją, ABI i liczbą sprawdzonych platform.

## Najważniejsze braki względem C

1. **Ograniczona przenośność.** Głównym celem jest x86-64. Drugi backend,
   AArch64 (Linux i bare metal pod QEMU), obsługuje większość programów
   testowych, ale brakuje RISC-V, innych formatów obiektowych niż ELF i
   obsługi Windows/PE-COFF.
2. **Brak dojrzałego standardowego środowiska.** `core` i `std` nie odpowiadają
   jeszcze libc: brakuje stabilnych kolekcji, stringów, plików, procesów,
   sieci, wątków, allocatorów i szerokiej dokumentacji API.
3. **Niepełny język.** `f32`/`f64`, 128-bitowe SIMD, bitfieldy i unie (w
   układzie C) już działają, ale generics, TLS, szersze wektory i część
   konstrukcji wieloplatformowych pozostają planowane albo są odrzucane przez
   `E0900`.
4. **Ograniczona interoperacyjność.** Są deklaracje `extern` i tryb object,
   ale nie ma jeszcze stabilnego, wieloplatformowego kontraktu porównywalnego z
   ekosystemem nagłówków C, varargs, bibliotekami systemowymi i narzędziami
   debugowania.
5. **Mniejsza dojrzałość optymalizatora.** Kod jest natywny, lecz nie należy
   oczekiwać jakości kodu GCC/Clang. Konserwatywne sloty ramek i ograniczony
   zestaw optymalizacji zwiększają rozmiar i mogą obniżać wydajność.
6. **Wąski zakres testów sprzętowych.** QEMU potwierdza ścieżkę bootowania i
   urządzenia emulowane przez QEMU. Nie potwierdza zachowania na różnych CPU,
   chipsetach, kontrolerach, BIOS/UEFI ani prawdziwym MMIO.
7. **Niestabilny kontrakt projektu.** Dokumentacja jest obszerna, ale część
   opisów historycznych i aktualnego statusu różni się między plikami. Zielony
   wynik `genesis/test.sh` jest bramką jakości każdego etapu.

## Ocena końcowa

Jako demonstracja własnego języka niskopoziomowego Oli-- jest bardzo mocne:
ma własny model pamięci, własny IR, własny backend, natywny kod i ścieżkę do
bootującego kernela. Jako narzędzie do zastąpienia C jest jeszcze za małe.

Najuczciwszy opis na GitHub to: **eksperymentalny, samohostujący język
systemowy dla x86-64 (i AArch64) z własnym backendem ELF i kontrolowanym
modelem pamięci**.
Nie należy jeszcze opisywać go jako kompletnego języka produkcyjnego ani jako
bezpieczniejszego zamiennika C bez zastrzeżeń.

## Źródła w repozytorium

- [README.md](../README.md)
- [docs/LANGUAGE.md](LANGUAGE.md)
- [docs/PROJECT_STATUS.md](PROJECT_STATUS.md)
- [docs/MACHINE_MODEL.md](MACHINE_MODEL.md)
- [docs/MEMORY_MODEL.md](MEMORY_MODEL.md)
- [docs/ABI.md](ABI.md)
- [docs/KERNEL_PROGRAMMING.md](KERNEL_PROGRAMMING.md)
