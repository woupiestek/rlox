cd test\%1
for /R %%f in (*) do (cargo run %%f)
