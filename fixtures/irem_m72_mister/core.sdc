# synthesized fixture MiSTer-style constraints
create_clock -name clk_sys -period 31.25 [get_ports clk_sys]
set_clock_groups -exclusive -group {clk_sys} -group {clk_ram}
