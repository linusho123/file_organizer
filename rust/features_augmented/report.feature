Feature: Summary report
  Every run ends with a report on stdout containing, in order: header,
  folders created, files moved, skipped items, issues, and totals.
  (PRD FR-15, FR-16)

  Scenario: The report lists all sections in order
    Given the workspace contains a file named "run01.storx"
    And the workspace contains a subfolder named "old_backups" containing a file named "backup.dmt"
    When I run the organizer on the target
    Then the report sections appear in order
    And the report contains "Organizing:"

  Scenario: Each move is reported as source -> destination
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the report contains "notes.txt  ->  TXT_Files/notes.txt"

  Scenario: The Issues section shows none when there are no issues
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the Issues section shows none

  Scenario: Skipped directories are listed with a reason
    Given the workspace contains a file named "notes.txt"
    And the workspace contains a subfolder named "old_backups" containing a file named "backup.dmt"
    When I run the organizer on the target
    Then the report contains "old_backups  (directory)"

  Scenario: Totals use singular forms for a count of one
    Given the workspace contains a file named "notes.txt"
    When I run the organizer on the target
    Then the report contains "Totals: 1 file moved, 1 folder created, 0 conflicts, 0 errors"
