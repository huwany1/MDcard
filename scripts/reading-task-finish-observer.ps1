# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Independent task-exit observation. No product hooks or position injection.
# Record the completed file read before parsing; preserve the exact raw bytes
# interpreted afterwards and retain separate parse/retry/poll timing.
Add-Type @'
using System;
using System.IO;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using System.Diagnostics;
using System.Collections.Generic;
public static class ReadingTaskFinishObserver {
 public static Task<string> Start(string path, long taskId) {
  var ready=new ManualResetEventSlim(false);
  var observer=Task.Run(() => {
   var clock=Stopwatch.StartNew();
   var polls=new List<object>();
   var failures=new List<object>();
   long previous=Stopwatch.GetTimestamp();
   ready.Set();
   while(clock.ElapsedMilliseconds<1500) {
    long begin=Stopwatch.GetTimestamp(),read=0,parsed=0;
    try {
     string raw;
     using(var file=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete))
     using(var reader=new StreamReader(file)) raw=reader.ReadToEnd();
     read=Stopwatch.GetTimestamp();
     using(var json=JsonDocument.Parse(raw)) {
      parsed=Stopwatch.GetTimestamp();
      polls.Add(new {begin_ticks=begin,read_completed_ticks=read,parse_completed_ticks=parsed,
       interval_ms=(begin-previous)*1000.0/Stopwatch.Frequency,
       read_ms=(read-begin)*1000.0/Stopwatch.Frequency,
       parse_ms=(parsed-read)*1000.0/Stopwatch.Frequency});
      foreach(var task in json.RootElement.GetProperty("background_tasks").EnumerateArray()) {
       if(task.GetProperty("task_id").GetInt64()!=taskId) continue;
       if(task.GetProperty("finished_monotonic_ns").GetUInt64()>0) {
        return JsonSerializer.Serialize(new {observed=true,task_id=taskId,raw=raw,
         read_completed_ticks=read,parse_completed_ticks=parsed,
         stopwatch_frequency=Stopwatch.Frequency,polls=polls,retries=failures,
         cancelled_while_running=task.GetProperty("cancel_requested_while_running").GetBoolean()});
       }
      }
     }
    } catch(IOException e) {failures.Add(new {ticks=Stopwatch.GetTimestamp(),kind=e.GetType().Name});}
      catch(JsonException e) {failures.Add(new {ticks=Stopwatch.GetTimestamp(),kind=e.GetType().Name});}
      catch(InvalidOperationException e) {failures.Add(new {ticks=Stopwatch.GetTimestamp(),kind=e.GetType().Name});}
      catch(KeyNotFoundException e) {failures.Add(new {ticks=Stopwatch.GetTimestamp(),kind=e.GetType().Name});}
    previous=begin;
    // Bound polling to at most once per millisecond without Windows' coarse
    // Sleep(1)/PowerShell retry quantum. Yield CPU while awaiting that deadline.
    while((Stopwatch.GetTimestamp()-begin)*1000.0/Stopwatch.Frequency<1) Thread.Yield();
   }
   return JsonSerializer.Serialize(new {observed=false,error="No completed task within 1500ms",polls=polls,retries=failures});
  });
  if(!ready.Wait(2000)) throw new TimeoutException("Task observer did not start");
  return observer;
 }
}
'@
