"""Guard publication against incomplete evidence and incorrect reported rates."""
import copy
import json
from pathlib import Path
import unittest
import publish

class EvidenceValidation(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.report=json.loads((Path(__file__).parents[1]/'results/2026-09-16-mac-mammoth.json').read_text())
 def check(self,report):publish.validate(report,[p[0] for p in publish.PHASES])
 def test_complete_evidence(self):self.check(self.report)
 def test_missing_and_duplicate_iteration_rejected(self):
  for mutation in ['missing','duplicate']:
   with self.subTest(mutation=mutation):
    report=copy.deepcopy(self.report)
    if mutation=='missing':report['samples'].pop()
    else:report['samples'][-1]=report['samples'][0]
    with self.assertRaises(ValueError):self.check(report)
 def test_inflated_throughput_rejected(self):
  report=copy.deepcopy(self.report);report['samples'][0]['aggregate_mib_s']*=2
  with self.assertRaises(ValueError):self.check(report)
 def test_different_workload_rejected(self):
  report=copy.deepcopy(self.report);report['options']['file_size']*=2
  with self.assertRaises(ValueError):self.check(report)
 def test_unverified_or_unclean_report_rejected(self):
  for field in ['verified','cleanup_complete']:
   report=copy.deepcopy(self.report);report[field]=False
   with self.assertRaises(ValueError):self.check(report)
 def test_missing_metric_stays_absent(self):
  self.assertIsNone(publish.aggregate(self.report,'unmeasured',1,'MiB/s'))
if __name__=='__main__':unittest.main()
