Feature: Recursive mode
  With --recursive, files at every depth below the input folder are pulled
  out of their subfolders and organized into the top-level type folders.
  Type folders themselves are destinations and are never traversed; emptied
  subfolders are left in place; undo restores nested files to their original
  folders. (PRD FR-28..FR-34)

  Scenario: Nested files are organized into top-level type folders
    Given the workspace contains a file named "top.md"
    And the workspace contains a nested file named "data/notes.txt"
    When I run the organizer on the target with --recursive
    Then the exit code is 0
    And the file "MD_Files/top.md" exists in the workspace
    And the file "TXT_Files/notes.txt" exists in the workspace
    And the file "data/notes.txt" does not exist in the workspace
    And the workspace contains a folder named "data"
    And the report contains "data/notes.txt  ->  TXT_Files/notes.txt"
    And the report contains "Totals: 2 files moved, 2 folders created, 0 conflicts, 0 errors"

  Scenario: Deeply nested files are found
    Given the workspace contains a nested file named "a/b/deep.storx"
    When I run the organizer on the target with --recursive
    Then the exit code is 0
    And the file "STORX_Files/deep.storx" exists in the workspace
    And the file "a/b/deep.storx" does not exist in the workspace

  Scenario: Without --recursive nested files are untouched
    Given the workspace contains a nested file named "data/notes.txt"
    When I run the organizer on the target
    Then the exit code is 0
    And the file "data/notes.txt" exists in the workspace
    And the workspace does not contain a folder named "TXT_Files"
    And the report contains "data  (directory)"

  Scenario: Type folders are destinations, never traversed
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "report.txt"
    And the workspace contains a nested file named "data/notes.txt"
    When I run the organizer on the target with --recursive
    Then the exit code is 0
    And the file "TXT_Files/report.txt" exists in the workspace
    And the report contains "TXT_Files  (type folder)"
    And the report contains "Totals: 1 file moved, 0 folders created, 0 conflicts, 0 errors"

  Scenario: Collisions across source folders get numeric suffixes
    Given the workspace contains a file named "report.txt" with content "top"
    And the workspace contains a nested file named "sub/report.txt" with content "nested"
    When I run the organizer on the target with --recursive
    Then the exit code is 0
    And the file "TXT_Files/report.txt" in the workspace has content "top"
    And the file "TXT_Files/report_1.txt" in the workspace has content "nested"
    And the report contains "Totals: 2 files moved, 1 folder created, 1 conflict, 0 errors"

  Scenario: A nested manifest file is never organized
    Given the workspace contains a nested file named "inner/.file_organizer_manifest.json"
    And the workspace contains a nested file named "inner/notes.txt"
    When I run the organizer on the target with --recursive
    Then the exit code is 0
    And the file "inner/.file_organizer_manifest.json" exists in the workspace
    And the workspace does not contain a folder named "JSON_Files"
    And the file "TXT_Files/notes.txt" exists in the workspace

  Scenario: Recursive dry run changes nothing
    Given the workspace contains a nested file named "data/notes.txt"
    When I run the organizer on the target with --recursive and --dry-run
    Then the exit code is 0
    And the workspace is unchanged
    And the report contains "DRY RUN - no changes made"
    And the report contains "data/notes.txt  ->  TXT_Files/notes.txt"

  Scenario: Undo restores nested files to their original folders
    Given the workspace contains a nested file named "data/notes.txt" with content "hello"
    When I run the organizer on the target with --recursive
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "data/notes.txt" in the workspace has content "hello"
    And the workspace does not contain a folder named "TXT_Files"
    And the file ".file_organizer_manifest.json" does not exist in the workspace

  Scenario: Undo recreates a source folder deleted after organizing
    Given the workspace contains a nested file named "data/notes.txt"
    When I run the organizer on the target with --recursive
    And the folder "data" is deleted from the workspace
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "data/notes.txt" exists in the workspace

  Scenario: Undo resolves collisions inside the original folder
    Given the workspace contains a nested file named "data/notes.txt" with content "original"
    When I run the organizer on the target with --recursive
    And the workspace gains a file named "data/notes.txt" with content "newcomer"
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "data/notes.txt" in the workspace has content "newcomer"
    And the file "data/notes_1.txt" in the workspace has content "original"
    And the report contains "conflict:"
