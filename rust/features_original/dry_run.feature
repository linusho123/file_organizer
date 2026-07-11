Feature: Dry-run mode
  With --dry-run the organizer prints exactly what a real run would do,
  including collision renames, but makes zero filesystem changes.
  (PRD FR-13, FR-14)

  Scenario: A dry run makes no filesystem changes
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --dry-run
    Then the exit code is 0
    And the workspace is unchanged
    And the workspace does not contain a folder named "STORX_Files"
    And the workspace does not contain a folder named "TXT_Files"

  Scenario: A dry run prints a banner and the planned moves
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a file named "notes.txt"
    When I run the organizer on the target with --dry-run
    Then the report contains "DRY RUN - no changes made"
    And the report contains "run01.storx"
    And the report contains "STORX_Files"
    And the report contains "notes.txt"
    And the report contains "TXT_Files"
    And the report contains "Totals: 2 files moved, 2 folders created, 0 conflicts, 0 errors"

  Scenario: A dry run previews collision renames without performing them
    Given the workspace contains a subfolder named "TXT_Files" containing a file named "report.txt" with content "first"
    And the workspace contains a file named "report.txt" with content "second"
    When I run the organizer on the target with --dry-run
    Then the exit code is 0
    And the workspace is unchanged
    And the report contains "conflict:"
    And the report contains "report_1.txt"
    And the file "TXT_Files/report_1.txt" does not exist in the workspace
    And the file "report.txt" in the workspace has content "second"

  Scenario: A real run does not print the dry-run banner
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the report does not contain "DRY RUN"
