Feature: Input validation
  The organizer accepts exactly one folder path and refuses to act on
  anything that is not an existing directory. (PRD FR-1..FR-4, FR-18)

  Scenario: The target path does not exist
    Given the target path does not exist
    When I run the organizer on the target
    Then the exit code is 2
    And stderr contains "Error: path does not exist:"

  Scenario: The target path is a file, not a directory
    Given the target path is a file
    When I run the organizer on the target
    Then the exit code is 2
    And stderr contains "Error: path is not a directory:"

  Scenario: An empty folder is a successful no-op
    When I run the organizer on the target
    Then the exit code is 0
    And the report contains "Totals: 0 files moved, 0 folders created, 0 conflicts, 0 errors"
