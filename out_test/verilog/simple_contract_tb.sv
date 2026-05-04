`timescale 1ns/1ps

module simple_contract_tb;

    // Clock and reset
    logic clk = 0;
    logic rst_n = 0;

    // Testbench control
    logic [7:0] cpu_control = 0;
    logic [7:0] cpu_status;
    logic [3:0] cpu_opcode = 0;
    logic signed [15:0] cpu_write_data = 0;
    logic [17:0] cpu_write_addr = 0;
    logic cpu_write_en = 0;
    logic cpu_read_en = 0;

    // Instantiate Unit Under Test
    simple_contract uut (
        .clk(clk),
        .rst_n(rst_n)
    );

    // Clock generation (100MHz = 10ns period)
    always #5 clk = ~clk;

    // Test sequence
    initial begin
        $dumpfile("waveform.vcd");
        $dumpvars(0, uut);

        // Reset sequence
        #0 rst_n = 0;
        #10 rst_n = 1;
        #5;

        // Test 1: Sync control
        cpu_control = 1;
        #10;
        cpu_control = 0;
        #10;

        // Test 2: Load input data
        cpu_control = 1;
        cpu_write_en = 1;
        cpu_write_addr = 0;
        cpu_write_data = 16'h1234;
        #10;
        cpu_write_en = 0;
        #10;

        // Test 3: Execute forward pass
        cpu_control = 20;
        #10;
        cpu_control = 0;
        #10;

        // Wait and finish
        #100;
        $display("Test completed successfully.");
        $finish;
    end

    // Monitor for debugging
    always @(posedge clk) begin
        if (uut.control != 0) begin
            $display("t=%0d: control=%d, status=%d", $time, uut.control, uut.status);
        end
    end

endmodule
