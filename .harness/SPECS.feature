Feature: Arkade in-process runtime executes compiled artifact JSON deterministically
  In order to validate compiler output end-to-end inside this repository
  As a runtime engineer
  I want a panic-free VM that executes `functions[].asm` with strict telemetry and debuggability

  Background:
    Given an artifact JSON with top-level keys:
      | key               |
      | contractName      |
      | constructorInputs |
      | functions         |
      | source            |
      | compiler          |
      | updatedAt         |
    And each function entry contains:
      | key           |
      | name          |
      | functionInputs|
      | serverVariant |
      | require       |
      | asm           |
    And `asm` is an ordered list of tokens where opcodes begin with "OP_"

  Scenario Outline: Stack manipulation opcode behavior
    Given the runtime stack is <initial_stack>
    When the VM executes a single opcode <opcode>
    Then execution result is "running"
    And the runtime stack becomes <expected_stack>
    Examples:
      | opcode  | initial_stack | expected_stack |
      | OP_DUP  | [1]           | [1,1]          |
      | OP_DROP | [1,2]         | [1]            |
      | OP_NIP  | [1,2,3]       | [1,3]          |

  Scenario Outline: Stack underflow is runtime failure (not script failure)
    Given the runtime stack is <initial_stack>
    When the VM executes a single opcode <opcode>
    Then execution result is "runtime_error"
    And runtime error code is "stack_underflow"
    Examples:
      | opcode  | initial_stack |
      | OP_DUP  | []            |
      | OP_DROP | []            |
      | OP_NIP  | [1]           |

  Scenario Outline: Arithmetic opcode behavior on script numbers
    Given the runtime stack is <initial_stack>
    When the VM executes a single opcode <opcode>
    Then execution result is "running"
    And the runtime stack becomes <expected_stack>
    Examples:
      | opcode   | initial_stack | expected_stack |
      | OP_ADD64 | [2,3]         | [5]            |
      | OP_SUB64 | [9,4]         | [5]            |
      | OP_MUL64 | [3,7]         | [21]           |
      | OP_DIV64 | [21,3]        | [7]            |

  Scenario: Arithmetic invalid input fails as runtime error
    Given the runtime stack is ["abc", 3]
    When the VM executes a single opcode OP_ADD64
    Then execution result is "runtime_error"
    And runtime error code is "invalid_numeric_encoding"

  Scenario: Division by zero fails as runtime error
    Given the runtime stack is [10,0]
    When the VM executes a single opcode OP_DIV64
    Then execution result is "runtime_error"
    And runtime error code is "division_by_zero"

  Scenario Outline: Cryptographic checks with standard opcodes
    Given execution bindings:
      | key        | value                           |
      | preimage   | 68656c6c6f                       |
      | hash_hex   | 2cf24dba5fb0a30e26e83b2ac5b9e29e |
      | pubkey     | 02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa |
      | signature  | 3044bbbbbbbbbbbbbbbbbbbbbbbbbbbb |
    And an asm program <asm_program>
    When the VM runs until halt
    Then execution result is <result_kind>
    And script truth value is <truth_value>
    Examples:
      | asm_program                                              | result_kind   | truth_value |
      | ["<preimage>","OP_SHA256","<hash_hex>","OP_EQUAL"]      | script_halt   | true        |
      | ["<preimage>","OP_SHA256","<hash_hex>","OP_EQUALVERIFY"]| script_halt   | true        |
      | ["<pubkey>","<signature>","OP_CHECKSIG"]                | script_halt   | true        |

  Scenario: Script failure is distinct from VM runtime failure
    Given an asm program ["1","2","OP_EQUAL","OP_VERIFY"]
    When the VM runs until halt
    Then execution result is "script_halt"
    And script truth value is false
    And no runtime error is recorded

  Scenario Outline: Non-interactive CLI execution
    Given the command `arkadec run <artifact_file>`
    When the command is invoked with `<args>`
    Then process exit code is <exit_code>
    And stdout contains "<stdout_substring>"
    And telemetry includes opcode-by-opcode stack state
    Examples:
      | artifact_file                 | args                                      | exit_code | stdout_substring |
      | examples/htlc.json            | --function claim --variant false          | 0         | RESULT: true     |
      | examples/htlc.json            | --function claim --variant false --trace  | 0         | OP_SHA256        |
      | examples/does_not_exist.json  | --function claim --variant false          | 2         | runtime_error    |

  Scenario Outline: TUI debugger initializes with required panes
    Given the command `arkadec debug <artifact_file>`
    When debugger is started with `<args>`
    Then terminal UI initializes successfully
    And pane "Script" is visible
    And pane "Main Stack / Alt Stack" is visible
    And pane "Telemetry Logs" is visible
    And controls include "Step Over", "Continue", and "Reset"
    Examples:
      | artifact_file      | args                             |
      | examples/htlc.json | --function claim --variant false |

