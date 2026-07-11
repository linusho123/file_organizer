Feature: Documented edge behaviors
  Behaviors the implementation already has but that no original scenario
  pinned down: trailing-dot classification (PRD FR-6), symlink and
  non-regular-file skipping (FR-5), the --version flag (FR-17), and a
  corrupt manifest on undo (FR-27). This feature exists to close those
  gaps so the contract documents the real behavior.

  Scenario Outline: A trailing dot leaves a file extensionless
    Given the workspace contains a file named "<filename>"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "NO_EXTENSION_Files/<filename>" exists in the workspace
    And the file "<filename>" does not exist in the workspace

    Examples:
      | filename     |
      | weird.       |
      | archive.tar. |

  Scenario: A top-level symlink is skipped, never moved
    Given the workspace contains a symlink named "shortcut.dat" pointing to "nowhere"
    When I run the organizer on the target
    Then the exit code is 0
    And the report contains "shortcut.dat  (symlink)"
    And the workspace does not contain a folder named "DAT_Files"

  Scenario: A non-regular file is skipped with a reason
    Given the workspace contains a fifo named "pipe.fifo"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the exit code is 0
    And the report contains "pipe.fifo  (not a regular file)"
    And the file "TXT_Files/notes.txt" exists in the workspace
    And the workspace does not contain a folder named "FIFO_Files"

  Scenario: The version flag prints the program name and exits cleanly
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --version
    Then the exit code is 0
    And the report contains "file-organizer"
    And the workspace does not contain a folder named "TXT_Files"

  Scenario: A corrupt manifest makes undo fail cleanly
    Given the workspace contains a file named "notes.txt"
    And the workspace contains a corrupt manifest
    When I run the organizer on the target with --undo
    Then the exit code is 2
    And stderr contains "Error: could not read manifest:"
