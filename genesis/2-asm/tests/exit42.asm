module test
proc main
    entry
    calls none
    machine x64
        bytes bf 2a 00 00 00      -- mov edi, 42
        bytes b8 3c 00 00 00      -- mov eax, 60
        bytes 0f 05               -- syscall
    end
end
