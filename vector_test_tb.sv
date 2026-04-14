`timescale 1ns/1ps

module vector_test_tb;

    logic clk;
    logic rst_n;


    // Instantiate Unit Under Test
    vector_test uut (
        .clk(clk),
        .rst_n(rst_n)
    );

    // Clock generation (100MHz)
    initial begin
        clk = 0;
        forever #5 clk = ~clk;
    end

    // Test Stimulus
    initial begin
        $dumpfile("waveform.vcd");
        $dumpvars(0, uut);

        rst_n = 0;
        #20 rst_n = 1;

        // Let it run for 1000ns
        #1000;
        $display("Simulation finished.");
        $finish;
    end

endmodule
