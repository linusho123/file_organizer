Feature: Undo the last organizing run
  Every run that moves at least one file records its moves in a manifest
  file (.file_organizer_manifest.json) inside the target folder. Running
  with --undo reverses the most recent recorded run: files move back to
  the top level and type folders created by that run are removed if they
  are left empty. (PRD FR-19..FR-27)

  Scenario: A run that moves files writes a manifest
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the file ".file_organizer_manifest.json" exists in the workspace

  Scenario: A dry run does not write a manifest
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --dry-run
    Then the file ".file_organizer_manifest.json" does not exist in the workspace

  Scenario: The manifest is never organized on later runs
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And I run the organizer on the target again
    Then the exit code is 0
    And the file ".file_organizer_manifest.json" exists in the workspace
    And the workspace does not contain a folder named "JSON_Files"
    And the report contains ".file_organizer_manifest.json  (manifest)"

  Scenario: Undo restores the original layout and removes created folders
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "run01.storx" exists in the workspace
    And the file "notes.txt" exists in the workspace
    And the workspace does not contain a folder named "STORX_Files"
    And the workspace does not contain a folder named "TXT_Files"
    And the file ".file_organizer_manifest.json" does not exist in the workspace
    And the report contains "Totals: 2 files restored, 2 folders removed, 0 conflicts, 0 errors"

  Scenario: Undo without a manifest fails cleanly
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --undo
    Then the exit code is 2
    And stderr contains "Error: no manifest found in:"

  Scenario: A run that moves nothing preserves the previous manifest
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And I run the organizer on the target again
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "notes.txt" exists in the workspace
    And the workspace does not contain a folder named "TXT_Files"

  Scenario: Undo resolves top-level collisions with a numeric suffix
    Given the workspace contains a file named "notes.txt" with content "original"
    When I run the organizer on the target
    And the workspace gains a file named "notes.txt" with content "newcomer"
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "notes.txt" in the workspace has content "newcomer"
    And the file "notes_1.txt" in the workspace has content "original"
    And the report contains "conflict:"
    And the report contains "notes_1.txt"
    And the report contains "Totals: 1 file restored, 1 folder removed, 1 conflict, 0 errors"

  Scenario: Undo reports missing recorded files as errors and keeps the manifest
    Given the workspace contains a file named "notes.txt"
    And the workspace contains a file named "run01.storx"
    When I run the organizer on the target
    And the file "TXT_Files/notes.txt" is deleted from the workspace
    And I run the organizer on the target with --undo
    Then the exit code is 1
    And the file "run01.storx" exists in the workspace
    And the report contains "error: could not restore"
    And the file ".file_organizer_manifest.json" exists in the workspace

  Scenario: Undo leaves a created folder that now contains other files
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And the workspace gains a file named "TXT_Files/keepme.txt" with content "user data"
    And I run the organizer on the target with --undo
    Then the exit code is 0
    And the file "notes.txt" exists in the workspace
    And the workspace contains a folder named "TXT_Files"
    And the file "TXT_Files/keepme.txt" exists in the workspace

  Scenario: A dry-run undo previews the restore without changing anything
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    And I run the organizer on the target with --undo and --dry-run
    Then the exit code is 0
    And the report contains "DRY RUN - no changes made"
    And the file "TXT_Files/notes.txt" exists in the workspace
    And the file ".file_organizer_manifest.json" exists in the workspace
    And the report contains "Totals: 1 file restored, 1 folder removed, 0 conflicts, 0 errors"
